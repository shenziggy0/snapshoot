use crate::error::Error;
use dirs::config_dir;
use reqwest_dav::{Auth, ClientBuilder, Depth};
use sqlx::{sqlite::SqliteConnectOptions, ConnectOptions, Connection};
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use walkdir::WalkDir;
use zip::read::ZipArchive;
use zip::write::SimpleFileOptions;

fn app_config_dir(app: &tauri::AppHandle) -> io::Result<PathBuf> {
    app.path_resolver()
        .app_config_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "无法定位应用配置目录"))
}

async fn snapshot_history(source: &Path, destination: &Path) -> Result<(), Error> {
    let destination = destination
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "数据库快照路径无法编码"))?;
    let mut database = SqliteConnectOptions::new()
        .filename(source)
        .read_only(true)
        .connect()
        .await?;
    // SQLite 从同一读事务读取主库和 WAL，已提交的记录会进入独立快照。
    sqlx::query("VACUUM INTO ?")
        .bind(destination)
        .execute(&mut database)
        .await?;
    database.close().await?;
    Ok(())
}

async fn build_backup(config_dir: &Path, output_dir: &Path) -> Result<NamedTempFile, Error> {
    let source = config_dir.join("history.db");
    let snapshot_dir = if source.try_exists()? {
        let directory = tempfile::tempdir()?;
        snapshot_history(&source, &directory.path().join("history.db")).await?;
        Some(directory)
    } else {
        None
    };

    let mut archive = NamedTempFile::new_in(output_dir)?;
    let archive_path = archive.path().canonicalize()?;
    let mut zip = zip::ZipWriter::new(archive.as_file_mut());
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("config.json", options)?;
    io::copy(&mut File::open(config_dir.join("config.json"))?, &mut zip)?;
    if let Some(directory) = snapshot_dir {
        zip.start_file("history.db", options)?;
        io::copy(
            &mut File::open(directory.path().join("history.db"))?,
            &mut zip,
        )?;
    }

    let plugins = config_dir.join("plugins");
    if plugins.try_exists()? {
        for entry in WalkDir::new(plugins) {
            let entry = entry?;
            if entry.path().is_file() {
                if entry.path().canonicalize()? == archive_path {
                    continue;
                }
                let name = entry
                    .path()
                    .strip_prefix(config_dir)?
                    .to_str()
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "插件路径无法编码")
                    })?;
                zip.start_file(name.replace('\\', "/"), options)?;
                io::copy(&mut File::open(entry.path())?, &mut zip)?;
            }
        }
    }
    zip.finish()?.sync_all()?;
    Ok(archive)
}

#[tauri::command(async)]
pub async fn webdav(
    app: tauri::AppHandle,
    operate: &str,
    url: String,
    username: String,
    password: String,
    name: Option<String>,
) -> Result<String, Error> {
    // build a client
    let client = ClientBuilder::new()
        .set_host(url.clone())
        .set_auth(Auth::Basic(username.clone(), password.clone()))
        .build()?;
    client.mkcol("/snapshoot").await.unwrap_or_default();
    let client = ClientBuilder::new()
        .set_host(format!("{}/snapshoot", url.trim_end_matches("/")))
        .set_auth(Auth::Basic(username, password))
        .build()?;
    match operate {
        "list" => {
            let res = client.list("/", Depth::Number(1)).await?;
            let result = serde_json::to_string(&res)?;
            Ok(result)
        }
        "get" => {
            let res = client.get(&format!("/{}", name.unwrap())).await?;
            let data = res.bytes().await?;
            let mut config_dir_path = config_dir().unwrap();
            config_dir_path = config_dir_path.join("top.shenziggy.snapshoot");
            let zip_path = config_dir_path.join("archive.zip");

            let mut zip_file = std::fs::File::create(&zip_path)?;
            zip_file.write_all(&data)?;
            let mut zip_file = std::fs::File::open(&zip_path)?;
            let mut zip = ZipArchive::new(&mut zip_file)?;
            zip.extract(config_dir_path)?;
            Ok("".to_string())
        }
        "put" => {
            let name = name.ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "缺少 WebDAV 备份文件名")
            })?;
            let config_dir = app_config_dir(&app)?;
            let archive = build_backup(&config_dir, &config_dir)
                .await?
                .into_temp_path();
            let data = std::fs::read(&archive)?;
            client.put(&format!("/{name}"), data).await?;
            Ok(String::new())
        }

        "delete" => match client.delete(&format!("/{}", name.unwrap())).await {
            Ok(()) => Ok("".to_string()),
            Err(e) => Err(Error::Error(format!("WebDav Delete Error: {}", e).into())),
        },
        _ => Err(Error::Error(
            format!("WebDav Operate Error: {}", operate).into(),
        )),
    }
}

#[tauri::command(async)]
pub async fn local(app: tauri::AppHandle, operate: &str, path: String) -> Result<String, Error> {
    match operate {
        "put" => {
            let destination = Path::new(&path);
            let output_dir = destination
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let config_dir = app_config_dir(&app)?;
            let archive = build_backup(&config_dir, output_dir).await?;
            // 先关闭临时归档，再替换目标。打包中断时旧备份仍可用。
            archive
                .into_temp_path()
                .persist(destination)
                .map_err(|error| error.error)?;
            Ok(String::new())
        }
        "get" => {
            let mut config_dir_path = config_dir().unwrap();
            config_dir_path = config_dir_path.join("top.shenziggy.snapshoot");

            let mut zip_file = std::fs::File::open(&path)?;
            let mut zip = ZipArchive::new(&mut zip_file)?;
            zip.extract(config_dir_path)?;
            Ok("".to_string())
        }
        _ => Err(Error::Error(
            format!("Local Operate Error: {}", operate).into(),
        )),
    }
}

#[tauri::command(async)]
pub async fn aliyun(operate: &str, path: String, url: String) -> Result<String, Error> {
    match operate {
        "put" => {
            reqwest::Client::new()
                .put(&url)
                .body(std::fs::read(&path)?)
                .send()
                .await?
                .error_for_status()?;
            Ok("".to_string())
        }
        "get" => {
            let res = reqwest::Client::new().get(&url).send().await?;
            let data = res.bytes().await?;
            let mut config_dir_path = config_dir().unwrap();
            config_dir_path = config_dir_path.join("top.shenziggy.snapshoot");
            let zip_path = config_dir_path.join("archive.zip");

            let mut zip_file = std::fs::File::create(&zip_path)?;
            zip_file.write_all(&data)?;
            let mut zip_file = std::fs::File::open(&zip_path)?;
            let mut zip = ZipArchive::new(&mut zip_file)?;
            zip.extract(config_dir_path)?;
            Ok("".to_string())
        }
        _ => Err(Error::Error(
            format!("Local Operate Error: {}", operate).into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_includes_committed_history_from_wal() {
        tauri::async_runtime::block_on(async {
            let config = tempfile::tempdir().unwrap();
            std::fs::write(config.path().join("config.json"), "{}").unwrap();
            let database_path = config.path().join("history.db");
            let mut database = SqliteConnectOptions::new()
                .filename(&database_path)
                .create_if_missing(true)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                .connect()
                .await
                .unwrap();
            sqlx::query("CREATE TABLE history(id INTEGER PRIMARY KEY, result TEXT NOT NULL)")
                .execute(&mut database)
                .await
                .unwrap();
            sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                .execute(&mut database)
                .await
                .unwrap();
            sqlx::query("INSERT INTO history VALUES (1, '已提交的译文')")
                .execute(&mut database)
                .await
                .unwrap();
            let primary_copy = config.path().join("primary-copy.db");
            std::fs::copy(&database_path, &primary_copy).unwrap();
            let mut primary = SqliteConnectOptions::new()
                .filename(primary_copy)
                .connect()
                .await
                .unwrap();
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM history")
                .fetch_one(&mut primary)
                .await
                .unwrap();
            assert_eq!(count, 0);
            primary.close().await.unwrap();

            let archive = build_backup(config.path(), config.path()).await.unwrap();
            let mut archive = ZipArchive::new(archive.into_file()).unwrap();
            let restored = tempfile::tempdir().unwrap();
            archive.extract(restored.path()).unwrap();
            let mut restored_database = SqliteConnectOptions::new()
                .filename(restored.path().join("history.db"))
                .read_only(true)
                .connect()
                .await
                .unwrap();
            let result: String = sqlx::query_scalar("SELECT result FROM history WHERE id = 1")
                .fetch_one(&mut restored_database)
                .await
                .unwrap();
            assert_eq!(result, "已提交的译文");
            restored_database.close().await.unwrap();
            database.close().await.unwrap();
        });
    }
}
