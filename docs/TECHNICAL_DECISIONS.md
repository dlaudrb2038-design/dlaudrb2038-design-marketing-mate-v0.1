# Phase 1 기술 결정

기준 문서는 `MARKETING_MATE_v0.1_FINAL.md`이며 이번 요청의 Phase 1 범위만 적용한다.
Phase 1 체크포인트에서 개발용 placeholder 경로 명칭을 `developer-placeholder`로 통일했다.

| 항목 | 선택·이유 |
|---|---|
| 런타임 | Node 24.19.0 / npm 11.9.0, Rust 1.99.0. 실제 현재 환경에서 사용한 안정 버전. Rust toolchain 고정. |
| 프런트엔드 | React/React DOM 19.3.0, TypeScript 7.0.2, Vite 8.3.3, React plugin 6.1.2, React types 19.3.0. 시작일 레지스트리의 안정 버전이며 TypeScript/Vite 빌드로 호환성을 검증. |
| Tauri | API/CLI/crate 2.12.1, build 2.7.1, single-instance 2.5.2. 요구된 Tauri 2를 유지하며 3.x alpha 제외. |
| SQLite | rusqlite 0.40.2 bundled+backup. Rust가 연결과 SQL을 소유하며 OS별 SQLite 차이를 줄이고 WAL을 포함한 migration 안전 사본 생성. |
| 기타 Rust | serde 1.0.229, serde_json 1.0.151, UUID 1.27.0, SHA-256 sha2 0.11.0, 테스트 tempfile 3.27.0. 직접 의존성 exact pin + Cargo.lock. |
| 버전 재현 | 직접 npm 버전 고정 + package-lock.json + npm ci, 직접 Cargo 버전 `=` 고정 + Cargo.lock + `--locked`. 자동 최신 버전 추적 없음. |
| 앱 데이터 | Tauri `app_data_dir()`와 고정 식별자 `com.marketingmate.desktop`. 프런트엔드에 DB 경로나 SQL을 노출하지 않음. 향후 식별자 변경 시 데이터 경로 이동 고려 필요. |
| 창 | pet / panel / settings 분리. pet 192 DIP·transparent·frameless·topmost·비포커스. panel 400×560, settings 960×720 (최소 760×560). |
| IPC 보안 | app_manifest에 명시한 typed commands만 등록, 창별 capability 분리. profile 읽기는 settings만 허용. shell/fs/sql/http/updater 플러그인 없음. 로컬 CSP, 원격 권한 웹뷰 없음. |
| Profile 최소 구현 | 명규·기본 말투·평일 09:00–18:00 KST. 지정되지 않은 보고 형식/선호 KPI/설정은 빈 값. singleton 제약과 JSON 타입 CHECK. character/growth/company 데이터 없음. |
| Migration | 연속 버전·SHA-256 ledger. 기존 DB 읽기 전용 검증 후 새 migration 필요 시 SQLite backup API로 안전 사본, migration+profile seed를 한 트랜잭션으로 적용. 미래 버전·체크섬 불일치·손상 거부. 이것은 Phase 8 사용자 암호화 백업 기능과 별개. |
| 종료 | 창 닫기는 숨김. tray 또는 typed quit 명령으로 종료 요청 후 WAL checkpoint/connection close. 단일 인스턴스 잠금은 DB setup 이전. |
| 리소스 | 공식 `assets/characters/developer-placeholder/` 골격만 생성. 제품 설계서와 동일한 명칭으로 통일. 지금은 스프라이트/manifest/loader 없음. 개발용 placeholder는 최종 개인화 에셋이 아님. |
| 설치기 | Phase 1 실행 파일만. bundle.active=false; NSIS 설치/업데이트·서명·제거는 Phase 8. updater 없음. |

Windows 11 x64가 제품 지원 대상이다. Linux 빌드와 실행은 클라우드 개발 검증으로만 보고하며
Windows 실제 실행·투명 합성·트레이 확인을 대신하지 않는다. SQLite는 암호화 DB가 아니다.
자격증명을 저장하거나 채팅·시트 데이터를 읽지 않으므로 Phase 1에 외부 키가 필요하지 않다.
