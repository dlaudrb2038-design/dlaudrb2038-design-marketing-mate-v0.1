import { useCallback, useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";

interface PersonalProfile {
  profile_id: string;
  user_name: string;
  tone: string;
  report_style: string | null;
  preferred_kpis_json: string;
  work_hours_json: string;
  settings_json: string;
}

interface AppStatus {
  database_ready: boolean;
  error_code: string | null;
}

type IconName = "profile" | "settings" | "window" | "database" | "arrow" | "check" | "info" | "exit";

function Icon({ name, className = "" }: { name: IconName; className?: string }) {
  const paths: Record<IconName, ReactNode> = {
    profile: <><circle cx="12" cy="8" r="3" /><path d="M5 20v-2a7 7 0 0 1 14 0v2" /></>,
    settings: <><path d="m9 3-1 3-3 1-2 3 2 2-1 4 3 2 3-1 3 1 3-2-1-4 2-2-2-3-3-1-1-3Z" /><circle cx="10.5" cy="11" r="3" /></>,
    window: <><rect x="3" y="4" width="18" height="16" rx="3" /><path d="M3 9h18M7 6.5h.01M10 6.5h.01" /></>,
    database: <><ellipse cx="12" cy="5" rx="8" ry="3" /><path d="M4 5v14c0 1.7 3.6 3 8 3s8-1.3 8-3V5M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3" /></>,
    arrow: <><path d="M5 12h14m-5-5 5 5-5 5" /></>,
    check: <path d="m5 12 4 4L19 6" />,
    info: <><circle cx="12" cy="12" r="9" /><path d="M12 11v6m0-10h.01" /></>,
    exit: <><path d="M10 4H5v16h5m-1-8h12m-4-4 4 4-4 4" /></>,
  };
  return <svg className={`icon ${className}`} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{paths[name]}</svg>;
}

function Brand({ compact = false }: { compact?: boolean }) {
  return <div className={`brand ${compact ? "brand--compact" : ""}`}>
    <span className="brand-mark" aria-hidden="true">m<span>·</span></span>
    <div><strong>MARKETING MATE</strong><span>나의 마케팅 파트너</span></div>
  </div>;
}

function parseJson(value: string): unknown {
  try { return JSON.parse(value) as unknown; } catch { return null; }
}

function formatWorkHours(value: string): string {
  const parsed = parseJson(value);
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return "—";
  const hours = parsed as Record<string, unknown>;
  const start = hours.start ?? hours.start_time;
  const end = hours.end ?? hours.end_time;
  if (typeof start !== "string" || typeof end !== "string") return "—";
  const zone = hours.timezone === "Asia/Seoul" || hours.timezone === "KST" ? "KST" : typeof hours.timezone === "string" ? hours.timezone : "";
  const days = hours.weekdays ?? hours.days;
  const weekdays = Array.isArray(days) && days.join(",") === "1,2,3,4,5" || days === "weekdays";
  return `${weekdays ? "평일 · " : ""}${start}–${end}${zone ? ` ${zone}` : ""}`;
}

function formatKpis(value: string): string {
  const parsed = parseJson(value);
  return Array.isArray(parsed) && parsed.length > 0 && parsed.every((item) => typeof item === "string") ? parsed.join(", ") : "미설정";
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return <div className="profile-field"><dt>{label}</dt><dd>{children}</dd></div>;
}

function Pet() {
  return <main className="pet-surface" aria-label="개발용 캐릭터 창">
    <div className="pet-placeholder" role="img" aria-label="개발용 임시 픽셀 도형. 최종 개인화 캐릭터가 아닙니다.">
      <div className="pet-ear pet-ear--left" /><div className="pet-ear pet-ear--right" />
      <div className="pet-face"><span className="pet-eye pet-eye--left" /><span className="pet-eye pet-eye--right" /><span className="pet-mouth" /></div>
      <div className="pet-foot pet-foot--left" /><div className="pet-foot pet-foot--right" />
    </div>
    <span className="development-label">DEVELOPMENT ASSET</span>
  </main>;
}

const futureSections = ["Workspace", "Data Source", "Field Mapping", "KPI Rules", "AI", "Character", "Notification", "Backup"];

export default function App({ desktop, windowLabel }: { desktop: boolean; windowLabel: string }) {
  const [profile, setProfile] = useState<PersonalProfile | null>(null);
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [loading, setLoading] = useState(desktop && windowLabel !== "pet");
  const [profileError, setProfileError] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    if (!desktop || windowLabel === "pet") return;
    setLoading(true);
    setProfileError(false);
    const results = await Promise.allSettled([
      windowLabel === "settings" ? invoke<PersonalProfile>("get_profile") : Promise.resolve(null),
      invoke<AppStatus>("get_app_status"),
    ]);
    const [profileResult, statusResult] = results;
    setProfile(profileResult.status === "fulfilled" ? profileResult.value : null);
    setProfileError(profileResult.status === "rejected");
    setStatus(statusResult.status === "fulfilled" ? statusResult.value : { database_ready: false, error_code: "IPC_UNAVAILABLE" });
    setLoading(false);
  }, [desktop, windowLabel]);

  useEffect(() => { void load(); }, [load]);

  const run = async (command: string) => {
    if (!desktop || busy) return;
    setBusy(true);
    setActionError(null);
    try { await invoke(command); }
    catch { setActionError("요청을 완료하지 못했습니다. 시스템 트레이에서 다시 시도해 주세요."); }
    finally { setBusy(false); }
  };

  if (windowLabel === "pet") return <Pet />;

  const databaseReady = status?.database_ready === true;
  const databaseLabel = !desktop ? "데스크톱 연결 필요" : loading ? "연결 확인 중" : databaseReady ? "로컬 DB 연결됨" : "로컬 DB 연결 실패";

  if (windowLabel === "panel") {
    return <main className="quick-panel">
      <Brand compact />
      <div className="panel-intro"><span className="eyebrow">PHASE 01 / FOUNDATION</span><h1>반가워요.</h1><p>MARKETING MATE의 기반을 준비했습니다.</p></div>
      <div className={`status-pill ${databaseReady ? "status-pill--ready" : ""}`} role="status"><Icon name={databaseReady ? "check" : "info"} />{databaseLabel}</div>
      <div className="panel-empty"><span className="panel-illustration" aria-hidden="true"><Icon name="window" /></span><h2>여기서 함께 시작해요</h2><p>이번 단계에서는 데스크톱 창과 개인 프로필을 확인할 수 있습니다.</p><p className="muted">성과 조회·대화·알림·성장은 후속 단계에서 구현합니다.</p></div>
      <button className="button button--primary button--wide" disabled={busy} onClick={() => void run("open_settings")}><Icon name="settings" />설정 열기<Icon name="arrow" /></button>
      <button className="button button--quiet button--wide" disabled={busy} onClick={() => void run("show_pet")}>캐릭터 창 표시</button>
      {actionError && <p className="error-message" role="alert">{actionError}</p>}
      <footer className="panel-footer">v0.1 · 개발용 기반 빌드</footer>
    </main>;
  }

  return <div className="settings-layout">
    <aside className="sidebar" aria-label="설정 탐색">
      <Brand />
      <div className="sidebar-heading">설정</div>
      <nav aria-label="설정 항목">
        <button className="nav-item nav-item--active" aria-current="page"><Icon name="profile" /><span>Personal Profile</span><span className="nav-dot" /></button>
        <div className="nav-section-label">후속 단계</div>
        {futureSections.map((section) => <button className="nav-item nav-item--disabled" disabled key={section}><span>{section}</span><span className="nav-dash" aria-hidden="true">—</span></button>)}
      </nav>
      <div className="sidebar-bottom"><span className="foundation-badge">PHASE 01</span><p>기반을 만드는 중입니다.</p><span>v0.1 · 개발 빌드</span></div>
    </aside>

    <main className="settings-main">
      <header className="page-header"><div><span className="breadcrumb">Settings <span>/</span> Personal Profile</span><h1>개인 프로필</h1><p>회사가 바뀌어도 함께하는 나의 기본 설정입니다.</p></div><span className="local-tag"><Icon name="database" />로컬 저장</span></header>

      {!desktop && <div className="notice notice--warning" role="status"><Icon name="info" /><div><strong>Tauri 데스크톱 앱에서 열어 주세요.</strong><p>현재는 브라우저 미리보기입니다. 개인 프로필과 로컬 데이터베이스에는 연결되지 않았습니다.</p></div></div>}
      {profileError && <div className="notice notice--warning" role="alert"><Icon name="info" /><div><strong>개인 프로필을 불러오지 못했습니다.</strong><p>아래의 로컬 저장소 상태를 확인하고 다시 시도해 주세요.</p></div></div>}

      <section className="card profile-card" aria-labelledby="profile-title" aria-busy={loading}>
        <div className="profile-identity"><div className="avatar" aria-hidden="true">{profile?.user_name.slice(0, 1) || "M"}</div><div><span className="eyebrow">PERSONAL PROFILE</span><h2 id="profile-title">{loading ? "프로필 불러오는 중" : profile ? `${profile.user_name}님의 프로필` : "개인 프로필"}</h2><p>개인 영역 · 회사 데이터와 분리</p></div><span className="readonly-tag">읽기 전용</span></div>
        <dl className="profile-fields">
          <Field label="이름">{profile?.user_name || "—"}</Field>
          <Field label="말투">{profile?.tone || "—"}</Field>
          <Field label="선호 보고 형식">{profile ? profile.report_style || "미설정" : "—"}</Field>
          <Field label="업무시간">{profile ? formatWorkHours(profile.work_hours_json) : "—"}</Field>
          <Field label="선호 KPI">{profile ? formatKpis(profile.preferred_kpis_json) : "—"}</Field>
        </dl>
        <div className="card-note"><Icon name="info" /><p>Phase 1에서는 저장된 기본 프로필을 확인합니다. 편집과 저장은 후속 단계에서 제공됩니다.</p></div>
      </section>

      <section className="card storage-card" aria-labelledby="storage-title"><div className="section-title"><span className="section-icon"><Icon name="database" /></span><div><h2 id="storage-title">로컬 저장소</h2><p>개인 프로필은 이 기기의 SQLite에 저장됩니다.</p></div></div><div className="storage-status"><div className={`status-pill ${databaseReady ? "status-pill--ready" : ""}`} role="status"><Icon name={databaseReady ? "check" : "info"} />{databaseLabel}</div>{status?.error_code && <span className="error-code">{status.error_code}</span>}<button className="text-button" disabled={!desktop || loading} onClick={() => void load()}>다시 확인</button></div></section>

      <section className="desktop-actions" aria-label="데스크톱 앱 동작"><div><h2>데스크톱 창</h2><p>숨긴 캐릭터는 시스템 트레이에서 다시 표시할 수 있습니다.</p></div><div className="action-buttons"><button className="button button--secondary" disabled={!desktop || busy} onClick={() => void run("show_pet")}>캐릭터 표시</button><button className="button button--secondary" disabled={!desktop || busy} onClick={() => void run("open_panel")}><Icon name="window" />패널 열기</button><button className="button button--quiet" disabled={!desktop || busy} onClick={() => void run("quit_app")}><Icon name="exit" />앱 종료</button></div></section>
      {actionError && <p className="error-message" role="alert">{actionError}</p>}
      <footer className="page-footer"><span>MARKETING MATE v0.1</span><span>개발용 기반 빌드 · 최종 개인화 에셋 미등록</span></footer>
    </main>
  </div>;
}
