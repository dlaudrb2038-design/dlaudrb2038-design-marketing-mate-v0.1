# Phase 1 Cloud 체크포인트 최종 검증 기록

검증일: 2026-10-07 (Asia/Seoul). 기준: `MARKETING_MATE_v0.1_FINAL.md`의
Phase 1과 이번 사용자의 명시적 체크포인트 정리 범위. 제품 설계서에는 사용자가
지정한 공식 placeholder 경로 정규화만 적용했고 아키텍처·보안·데이터·기능 범위와
192×192 DIP 계약은 유지했다. 새 제품 기능이나 Phase 2 구현은 추가하지 않았다.
첨부 원문 SHA-256: `2ad9bc7b6b47f59fc93c8bbda4dc45a6376095b537711156f2fe5f9e82a4b36c`.
경로 정리 후 프로젝트 기준 문서 SHA-256: `fac3c59af30ccbf99b6039cde2a24e91909dd0a0eb6c089df4435a8da6a74257`.

## 생성/수정 파일

수정: `README`.

이번 정리에서 변경한 파일은 `README`, `assets/characters/developer-placeholder/README.md`,
`docs/MARKETING_MATE_v0.1_FINAL.md`, `docs/TECHNICAL_DECISIONS.md`, 이 검증 기록,
`scripts/smoke_desktop.py`다. 테스트 스크립트 변경은 크기 조사용 관찰값
(devicePixelRatio, body min-width, pet surface 크기) 기록만 추가했다.
아래 기존 Phase 1 산출물을 모두 체크포인트 커밋에 포함한다.

생성:

- `.gitattributes`, `.gitignore`
- `package.json`, `package-lock.json`, `tsconfig.json`, `vite.config.ts`, `rust-toolchain.toml`, `index.html`
- `src/main.tsx`, `src/App.tsx`, `src/styles.css`, `src/vite-env.d.ts`
- `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`
- `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/desktop.rs`, `src-tauri/src/storage.rs`
- `src-tauri/migrations/0001_personal_profile.sql`
- `src-tauri/capabilities/pet.json`, `src-tauri/capabilities/panel.json`, `src-tauri/capabilities/settings.json`
- `src-tauri/icons/icon.png`, `src-tauri/icons/icon.ico` (개발용 앱/트레이 표식; 개인화 캐릭터 아님)
- `assets/characters/developer-placeholder/README.md`
- `scripts/smoke_desktop.py`, `scripts/run_linux_smoke.sh`
- `docs/MARKETING_MATE_v0.1_FINAL.md`, `docs/TECHNICAL_DECISIONS.md`, 이 검증 기록

빌드 출력 `dist/`, `src-tauri/target/`, Tauri 생성 schema/permission,
검증 실행 산출물 `.verification/`는 ignore한다. 클라우드용 Rust/네이티브 패키지는
체크아웃 밖 `/workspace/.tools/`, 캐시는 `/workspace/.cache/`에 있다.

## 구현된 Phase 1 항목

| 항목 | 구현 |
|---|---|
| 프로젝트 | Tauri 2 + React + TypeScript + Rust, exact 버전과 두 lockfile |
| 데스크톱 구조 | Windows용 main subsystem, 고정 앱 식별자, pet/panel/settings 창 분리 |
| 캐릭터 창 | 192 DIP 설정·CSS 캔버스, 투명·frameless·topmost, 시작 시 비포커스, 주 모니터 작업영역 우하단 배치 |
| 트레이 | 패널·설정·캐릭터 표시/숨김·명시적 종료 메뉴; 닫기는 숨김 |
| 단일 인스턴스 | SQLite setup 이전 plugin 등록, 두 번째 실행은 기존 패널 표시 |
| SQLite | Rust 소유 연결, FK ON/WAL/busy timeout 5초, migration ledger/checksum, transaction, 기존 DB의 migration 안전 사본 |
| Personal Profile | UUID 단일 프로필, 지정된 기본 이름·말투·KST 업무시간, 빈 선호/설정; 읽기 전용 |
| Settings | 한국어 shell, 실제 프로필/DB 상태, 진단 상태에서도 접근 가능, 후속 항목 비활성 |
| 리소스 | 공식 assets/characters/developer-placeholder/ 경로 골격; CSS 임시 도형에 DEVELOPMENT ASSET 표시 |
| 보안 기반 | 창별 capability, 고정 typed IPC, 로컬 CSP, SQL·임의 경로·외부 페이지·비밀 입력 없음 |

## 실행 결과

호스트: Linux x86_64, Debian 13; cgroup CPU 4 cores / RAM 32 GiB.
Node 24.19.0, npm 11.9.0, Rust 1.99.0. GTK 3.24.49 / WebKitGTK 2.54.0 /
Ayatana appindicator 0.5.94를 사용자 디렉터리에 설치했다. 공식 Rust SHA-256,
Debian apt 서명/패키지 checksum, npm/Cargo integrity 검증을 유지했다.
권한 없는 시스템 설치 대신 로컬 sysroot와 helper 경로용 proot를 사용했다.
WebKit sandbox 비활성화는 사용하지 않았다.

`npm run desktop:build:debug`가 성공했고 실제 native 실행 파일
`src-tauri/target/debug/marketing-mate`를 Xvfb + D-Bus + WebKitWebDriver로 실행했다.
실제 React DOM과 Rust IPC를 통한 Settings 프로필/DB 연결을 확인했다.
명시적 종료 후 프로세스가 사라지고 재실행 시 같은 UUID가 유지됐다.
브라우저 화면은 Tauri 미연결 상태를 명시하며 DB가 연결된 것처럼 표시하지 않는다.

## 테스트 결과

| 검증 | 결과 |
|---|---|
| `npm ci` | 재실행 성공, exit 0; `.verification/checkpoint/npm-ci.log` |
| `npm run build` / 내부 `npm run typecheck` | 재실행 성공, strict tsc + Vite, exit 0; `frontend-build.log` |
| `cargo check --locked --manifest-path src-tauri/Cargo.toml` | 재실행 성공, exit 0; `cargo-check.log` |
| `npm run desktop:build:debug` | 재실행 성공, 실제 실행 파일 생성, exit 0; `desktop-build.log` |
| `npm run test:rust` | 재실행 10 passed / 0 failed / 0 ignored / 0 filtered, exit 0; `rust-tests.log` |
| `cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | 재실행 성공, exit 0; `clippy.log` |
| `cargo fmt --check --manifest-path src-tauri/Cargo.toml` | 성공, exit 0; `cargo-fmt.log` |
| `bash scripts/run_linux_smoke.sh` | 최종 정적 빌드 재실행 19 passed / 0 failed, exit 0; `.verification/linux-smoke-WHTuqb/results.json` |
| `bash scripts/run_linux_smoke.sh --development-only` | 재실행 4 passed / 0 failed, CLI exit 0, Vite 종료 확인; `.verification/linux-smoke-Q9GcK1/results.json` |

개별 명령 로그는 별도 전체 경로가 없는 경우 `.verification/checkpoint/` 아래에 있다.
로그를 캡처한 명령은 실제 명령의 exit status를 보존했다. 아래 결과는 이번 정리 이후
실행한 결과이며 이전 작업의 결과를 재실행 결과로 대체하지 않았다.

DB 테스트는 첫 실행과 재실행, singleton/JSON CHECK, 필수 pragmas, CRLF/LF checksum,
프로필 누락 진단, 미래 schema/수정 checksum 시 원본 보존, 손상 파일 보존,
WAL 포함 사전 사본과 migration 실패 rollback, 성공 upgrade, unversioned DB 거부를 실행했다.
테스트 안의 가상 후속 migration은 안전성 검증용이며 제품 테이블/기능으로 등록하지 않았다.
main/doc-test의 0 tests는 별도 기능 검증으로 계산하지 않았다.

실제 smoke는 pet 투명 HTML/body·192 CSS canvas·개발 표시, settings의 실제 DB/profile IPC,
pet/panel의 profile 거부, 임의 SQL/filesystem 명령 거부, hide/show, 단일 프로세스,
종료·재실행과 UUID 보존, SQLite integrity/singleton/ledger를 확인했다.
네이티브 window property에서 frameless/above/skip-taskbar, 닫기 후 숨김과
두 번째 실행의 기존 패널 복원을 추가 확인했다. Linux native DBus 트레이 메뉴
5개와 실제 callback(숨김/표시/설정/패널/종료)을 실행했다.
고립된 테스트 DB 손상 시 Settings 자동 진단 진입·DB_CORRUPT·원본 byte 보존·종료를
확인한 뒤 정상 fixture를 복원했다. 실제 사용자 DB는 이 테스트에서 사용하지 않는다.
스크립트에는 제품용 테스트 명령이나 별도 mock backend가 없다.

클라우드에서 재현하려면 `npm run desktop:build:debug` 이후
`bash scripts/run_linux_smoke.sh`를 실행한다. runner는 retained 로컬 도구를 사용해
고유 `.verification/linux-smoke-*` 출력·개인 D-Bus/Xvfb 세션·fixture 데이터 경로를 만들고
자신이 시작한 프로세스만 정리한다. StatusNotifier fixture는 운영체제 데스크톱 host의
검증용 대역이며, 제품의 트레이 메뉴와 callback은 실제 Tauri 구현을 사용한다.

개발 모드만 재현할 때는 `bash scripts/run_linux_smoke.sh --development-only`를 실행한다.
이는 `npm run desktop:dev`를 그대로 실행하고 Vite를 통한 실제 native React와
Rust profile/DB IPC를 확인한다. Node/Vite/Cargo는 proot 밖에서 실행하고 Cargo의
플랫폼 runner 설정으로 네이티브 앱에만 WebKit helper 경로를 적용한다.
개발 CLI 종료 코드 0과 Vite 1420 정리도 확인했다. 초기 검증 도구가 CLI 완료 전
세션을 정리해 beforeDevCommand 오류를 남겼으나, CLI 종료까지 기다리도록 수정한
최종 실행에서는 해당 오류가 없었다. 단독 실행 파일은 개발 모드 검증 뒤 정적
디버그 빌드로 다시 만들어 Vite 없이 실행 가능한 상태로 남겼다.

## 미구현 항목

Phase 2 이후 전체: atlas/manifest loader·20 clip·5상태/4방향·드래그·클릭 판정·클릭 통과,
Workspace·Sheets/OAuth·OpenAI·KPI·Alert·Growth·대화·백업/복구 UI·NSIS 설치/업데이트·서명.
Profile 편집, 캐릭터 위치/배율 저장 및 모니터 변경 대응도 이번 최소 기반 shell에는 없다.
정식 개인화 Character Master, 릴리스 P0 전체·성능 SLA를 통과했다고 주장하지 않는다.

## Placeholder 경로 정리 결과

공식 경로는 `assets/characters/developer-placeholder/`다. 제품 설계서 18.1,
Phase 2 산출물 표, D03, README, 기술 결정 및 리소스 설명에서 동일 명칭으로 통일했다.
Tauri `bundle.resources`의 프로젝트 소스 경로와 번들 목적 경로도 이 경로와 일치한다.
최종 디버그 실행 파일 옆에 복사된 공식 리소스 README가 프로젝트 원본과 동일함을 확인했다.
코드·설정·문서·검증 스크립트의 기존 축약 경로 명칭은 제거했다.
생성 출력·의존성·Git 메타데이터를 제외한 프로젝트 검색에서 기존 축약 명칭 0건을 확인했다.
Phase 1에는 캐릭터 `manifest.json` 파일이나 로더가 없으며 새로 생성하지 않았다.
캐릭터의 20 clip·atlas/manifest 구현은 Phase 2에 남아 있다.

## 192×192 DIP 조사 결과

제품 계약과 Tauri pet window config의 width/height는 192다. Rust 초기 위치 계산에도
192를 사용하며, pet의 html/body/#root 및 .pet-surface CSS는 192px다.
공통 body min-width=320px는 pet 선택자의 min-width=0으로 덮어쓴다.
이번 실제 WebDriver 측정은 devicePixelRatio=1, body min-width=0px,
root/surface=192×192px, viewport=200×200이었다.

실제 앱의 GTK widget 크기를 격리된 실행에서 읽기용 진단 callback으로 측정했다.
이 진단은 앱 코드/바이너리를 수정하지 않았으며 종료 acceptance 검증으로 세지 않았다.

| 실제 앱의 native 측정 항목 | 값 |
|---|---|
| GtkApplicationWindow default size | 192×192 |
| resizable / scale factor | false / 1 |
| GtkApplicationWindow allocation / natural size | 200×200 / 200×200 |
| GtkBox natural size | 0×0 |
| WebKitWebView natural size / explicit size request | 0×0 / -1×-1 |

원인은 현재 Linux GTK 고정 크기 창의 geometry 협상이다. WebKit 자식의 natural size가
0이고 명시적인 native size request가 없을 때 GTK는 natural size를 fallback 200으로
정한다. resizable=false 창에서는 이 natural size 기반으로 고정 min/max geometry를
만들어 default size=192여도 native allocation이 200이 된다.

동일 HTML/CSS의 외부 진단 probe로 대조했다. 앱 설정에 이 대조 조건을 적용하지 않았다.

| 진단 조건 (default size 192, CSS 192 유지) | native / JS viewport |
|---|---|
| fixed 창, 기본 WebView size request | 200×200 / 200×200 |
| resizable 창 | 192×192 / 192×192 |
| fixed 창, native WebView size request=192 | 192×192 / 192×192 |

소스 근거:

- [GTK 3.24.49 gtkwindow.c:152](https://github.com/GNOME/gtk/blob/3.24.49/gtk/gtkwindow.c#L152): `NO_CONTENT_CHILD_NAT=200`.
  [8876](https://github.com/GNOME/gtk/blob/3.24.49/gtk/gtkwindow.c#L8876),
  [9011](https://github.com/GNOME/gtk/blob/3.24.49/gtk/gtkwindow.c#L9011): 자식 natural size=0의 fallback.
  [10379](https://github.com/GNOME/gtk/blob/3.24.49/gtk/gtkwindow.c#L10379): non-resizable geometry 추정.
- Tao 0.37.1 `src/platform_impl/linux/window.rs:110,123`: default size 설정 후 첫 configure에서 resizable=false 적용.
- Tauri runtime Wry 2.12.1 `src/lib.rs:5198`: GtkBox 사용.
  Wry 0.57.0 `src/webkitgtk/mod.rs:694,708`: GtkBox에서는 pack_start, GtkFixed에서만 bounds를 size request로 적용.
- [WebKit 2.54.0 WebKitWebViewBase.cpp:1058](https://github.com/WebKit/WebKit/blob/webkitgtk-2.54.0/Source/WebKit/UIProcess/API/gtk/WebKitWebViewBase.cpp#L1058): 최소 크기=0, natural 크기=contentsSize.

Wry의 기본 bounds=200은 현재 GtkBox 분기에서 native size request로 적용되지 않는다.
따라서 보편적인 WebKit 최소 크기 200으로 해석하지 않는다. 현재 GTK/WebKit/Tao 조합의
확인된 Linux 한계이며 제품 계약, Tauri config, CSS를 200으로 변경하지 않았다.
진단 코드·원문 소스·측정 로그는 `/workspace/.tools/investigation/`에 있다.
Windows WebView2 경로는 GTK를 사용하지 않으며 Windows 실제 크기 검증은 별도로 남는다.

## Linux 한계와 검증 제한

- Linux WebKit 네이티브 viewport가 200×200이었다. Tauri 설정과 CSS canvas는 192×192다.
  공식 대상인 Windows에서 실제 DIP 크기를 확인하기 전 192 native 창 검증 완료로 판단하지 않는다.
- 초기 headless Linux stalonetray fallback은 X11 BadWindow로 표시가 실패했다.
  native StatusNotifier DBus host fixture로 메뉴 등록·callback 검증은 통과했다.
  렌더링된 Windows 시스템 트레이의 실제 마우스 동작/접근성은 미검증이다.
- 초기 GTK Gdk 경고는 XDG 디렉터리를 D-Bus 이전에 설정한 재현 runner에서 관찰되지
  않았다. Ayatana deprecated 경고는 남아 있으며 기능 실패는 관찰되지 않았다.
- 초기 개발 모드 검증은 proot 아래의 중첩 Node 파일 I/O에서 EFAULT로 시작 전 실패했다.
  Node/Vite/Cargo를 proot 밖에서 실행하고 네이티브 앱에만 helper bind를 적용해
  개발 화면과 실제 IPC 검증은 통과했다. `UV_USE_IO_URING=0`만으로는 해결되지 않았으며
  검증 runner에 호환 옵션과 분리 실행을 기록했다. 제품 설정 변경은 없다.
- Windows 11 x64 컴파일/실행, WebView2, 투명 합성, 실제 시스템 트레이,
  DPI·다중 모니터 동작은 이 Linux 환경에서 실행하지 않았다.

## 다음 Phase 전 필요한 사항

Windows 11 x64에서 README의 준비·명령으로 build/test 실행 후 아래 체크리스트를 수행한다.
이 항목은 실제 확인 전 미완료다. 외부 API 자격증명은 Phase 1 검증에 필요하지 않다.

- [ ] 첫 실행 시 캐릭터 창 192×192 DIP, 투명 합성·frameless·topmost·비포커스 확인
- [ ] 트레이에서 패널·설정 진입, 숨김 후 복원, 트레이 종료 확인
- [ ] 설정창 readonly 기본 프로필과 DB 연결 상태 확인
- [ ] 창 닫기가 숨김이고 트레이에서 다시 열 수 있는지 확인
- [ ] 두 번째 실행이 기존 패널을 열고 프로세스/프로필이 중복되지 않는지 확인
- [ ] 명시적 종료·재실행 시 UUID/기본 프로필 유지 및 SQLite integrity 확인
- [ ] 손상/미래 schema의 별도 테스트 데이터에서 원본 보존·설정 진단 확인
- [x] placeholder 공식 경로 명칭 정합 확인

D03 정식 Character Master는 릴리스 전에 필요하다. placeholder는 최종 에셋으로 취급하지 않는다.
D01/D02/D04/D05/D06은 각각 해당 이후 Phase의 실제 연결·배포 시점에 준비한다.

## git 체크포인트

검증 완료 후 기존 Phase 1 산출물과 이번 정리를 함께 커밋한다.
커밋 메시지: `feat: complete marketing mate phase 1 foundation`.
기준 HEAD: `5550dfeb81d92c90bfee107bc0a19bba8e94cc67`.
커밋 자체의 정확한 SHA와 최종 clean status는 작업 완료 보고 및 `git log -1` /
`git status --short`로 확인한다. 검증 로그·캐시·빌드 출력은 커밋에서 제외한다.

## Cloud 체크포인트 판정

Linux Cloud에서 요구된 재설치·build/typecheck·cargo check·Clippy·Rust tests·기존
Phase 1 네이티브/개발 모드 검증을 모두 통과했다. 공식 placeholder 경로 정리와
192×192 DIP 조사 결과 및 확인된 Linux 한계를 기록했다. Cloud 체크포인트 판정은 PASS다.
Windows 11 x64 공식 환경의 빌드·실행·192 DIP/투명 합성·실제 트레이 마우스 동작 검증은
남아 있다. Windows 검증이나 전체 릴리스 P0 합격을 완료 처리하지 않았다.

## 재사용 클라우드 설정

설치 갱신 명령 `install_script`와 시작/검증 지침 `start_skill`을 환경 설정 초안으로
저장했다(status=saved). 저장 자체는 스크립트 실행·런타임 적용·게시가 아니다.
재사용하려면 환경 설정에서 초안을 검토·저장한 뒤 환경을 게시해야 한다.
현재 인스턴스 검증만 수행했으며 게시 또는 새 task 복원 검증은 수행하지 않았다.
