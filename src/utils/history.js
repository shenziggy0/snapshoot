import Database from 'tauri-plugin-sql-api';

let historyDatabase;

export function openHistory() {
    if (!historyDatabase) {
        historyDatabase = Database.load('sqlite:history.db')
            .then(async (db) => {
                await db.execute(`CREATE TABLE IF NOT EXISTS history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    text TEXT NOT NULL,
                    source TEXT NOT NULL,
                    target TEXT NOT NULL,
                    service TEXT NOT NULL,
                    result TEXT NOT NULL,
                    timestamp INTEGER NOT NULL
                )`);
                return db;
            })
            .catch((error) => {
                historyDatabase = undefined;
                throw error;
            });
    }
    return historyDatabase;
}
