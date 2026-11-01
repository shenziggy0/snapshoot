# Snapshoot

Snapshoot는 Windows와 macOS에서 사용할 수 있는 크로스플랫폼 스크린샷 도구입니다. 스크린샷을 찍은 뒤 바로 주석을 달고, 화면에 고정하고, 긴 스크린샷을 찍고, OCR로 문자를 인식하고, 번역까지 한 번에 할 수 있습니다.

## 기능

- **스크린샷**: 원하는 영역을 선택해 캡처합니다. 여러 모니터를 지원합니다.
- **주석**: 스크린샷 위에 상자, 화살표, 선, 모자이크, 텍스트를 바로 그릴 수 있고, 자르기와 되돌리기도 지원합니다.
- **고정**: 스크린샷을 다른 창 위에 고정해 두고 보면서 작업할 수 있습니다. 크기와 투명도를 조절할 수 있습니다.
- **긴 스크린샷**: 페이지를 자동으로 스크롤하면서 이어 붙입니다. 웹페이지나 긴 문서에 유용합니다.
- **OCR**: 이미지 속 문자를 인식합니다. 로컬 OCR과 온라인 OCR 서비스를 지원합니다.
- **번역**: 선택 영역 번역, 입력 번역, 스크린샷 번역, OCR 번역을 지원하며, 설정에서 여러 번역 서비스를 추가하고 바꿀 수 있습니다.
- **기록**: 번역과 인식 기록을 검색하고 내보낼 수 있습니다.
- **백업**: 설정을 로컬, WebDAV, 알리윤 드라이브에 백업할 수 있습니다.
- **외부 호출**: 같은 컴퓨터의 다른 앱이 `http://127.0.0.1:60828`을 통해 번역, OCR 등을 호출할 수 있습니다. 포트는 설정에서 바꿀 수 있습니다.

## 단축키

모든 단축키는 설정 -> 단축키에서 바꿀 수 있습니다. 기본값은 다음과 같습니다.

| 기능 | macOS | Windows |
| --- | --- | --- |
| 스크린샷 주석 | Command + Shift + A | F1 |
| 클립보드 이미지 고정 | Command + Shift + P | F3 |
| 선택 영역 번역 | 설정되지 않음 | 설정되지 않음 |
| 입력 번역 | 설정되지 않음 | 설정되지 않음 |
| OCR 인식 | 설정되지 않음 | 설정되지 않음 |
| OCR 번역 | 설정되지 않음 | 설정되지 않음 |

선택 영역, 입력, OCR 단축키는 기본값이 비어 있습니다. 처음 사용할 때 편한 키 조합을 설정해 주세요.

## 다운로드 및 설치

설치 파일은 GitHub Releases 페이지에서 받을 수 있습니다.

https://github.com/shenziggy0/snapshoot/releases

v1.0.0은 설치 파일 세 개를 제공합니다.

| 시스템 | 파일 |
| --- | --- |
| Windows 64비트 | `Snapshoot_1.0.0_x64-setup.exe` |
| macOS Apple silicon (M 시리즈 칩) | `Snapshoot_1.0.0_aarch64.dmg` |
| macOS Intel | `Snapshoot_1.0.0_x64.dmg` |

정확한 파일 이름은 Release에 첨부된 파일을 기준으로 합니다.

### Windows

`Snapshoot_1.0.0_x64-setup.exe`를 실행하고 설치 마법사를 따라 주세요.

### macOS

먼저 칩을 확인하세요. 애플 메뉴 -> 이 Mac에 관하여에서 "칩" 항목을 확인합니다. Apple M 시리즈인지 Intel인지에 맞는 dmg를 받으세요.

1. dmg를 열고 Snapshoot를 응용 프로그램 폴더로 드래그합니다.
2. 처음 실행할 때 macOS가 개발자를 확인할 수 없다고 할 수 있습니다. 설치 파일이 Apple 서명과 공증을 거치지 않았기 때문이며, 파일이 손상되었다는 뜻은 아닙니다.
3. 한 번만 허용하면 이후에는 정상적으로 열 수 있습니다.
   - Finder -> 응용 프로그램에서 Snapshoot를 Control-클릭 -> 열기 -> 대화상자에서 "열기"를 클릭합니다.
   - 이미 더블클릭해서 차단되었다면, 시스템 설정 -> 개인정보 보호 및 보안에서 아래쪽의 "그래도 열기"를 클릭합니다.
   - 그래도 열리지 않을 때만 터미널에서 `xattr -cr /Applications/Snapshoot.app`을 실행해 격리 표시를 지우고 다시 엽니다.
4. 처음 스크린샷을 찍거나 단축키를 쓸 때 macOS가 권한을 요청합니다. 시스템 설정 -> 개인정보 보호 및 보안에서 Snapshoot에 "화면 기록"과 "손쉬운 사용" 권한을 켜야 스크린샷과 단축키가 동작합니다.

## 개인정보 안내

번역과 OCR 서비스 키는 설정에서 직접 입력하며 로컬에 저장됩니다. Snapshoot 서버로 업로드되지 않습니다. 기본 번역 서비스는 키가 필요 없는 무료 서비스입니다.

다음 세 가지는 켜기 전에 꼭 확인해 주세요.

- Lingva 번역: 번역할 텍스트가 `lingva.pot-app.com`으로 전송됩니다. Snapshoot 서버가 아니라 상류 서버입니다.
- ECDICT 사전: 찾을 텍스트가 `pot-app.com/api/dict`으로 전송됩니다. 역시 상류 서버입니다.
- 알리윤 드라이브 백업: 로그인 토큰을 교환할 때 `pot-app.com/api/ali_access_token`을 거칩니다. 역시 상류 서버입니다.

이 세 가지는 기본적으로 켜져 있지 않습니다. 설정에서 직접 선택해야만 사용됩니다.

## 소스에서 빌드하기

소스에서 직접 빌드할 수도 있습니다. Node.js 21, pnpm 9, Rust stable이 필요합니다.

```bash
git clone https://github.com/shenziggy0/snapshoot.git
cd snapshoot
pnpm install
pnpm tauri dev
```

`pnpm tauri dev`는 개발 모드로 실행합니다. 패키징은 다음과 같습니다.

```bash
pnpm tauri build
```

빌드 결과물은 `src-tauri/target/release/bundle/` 아래에 생성됩니다. 특정 대상만 빌드하려면, 예를 들어 다음과 같이 합니다.

```bash
pnpm tauri build --target aarch64-apple-darwin --bundles dmg
pnpm tauri build --target x86_64-apple-darwin --bundles dmg
pnpm tauri build -b nsis --target x86_64-pc-windows-msvc
```

## 피드백

버그나 제안 사항은 GitHub Issues에 남겨 주세요.

https://github.com/shenziggy0/snapshoot/issues

감사의 말: Snapshoot는 Pot (pot-app/pot-desktop)을 기반으로 하며, GPL-3.0 라이선스를 따릅니다.
