import type { LogEntry } from "../lib/types";
import { useI18n } from "../lib/i18n";

export function LogViewer({ logs }: { logs: LogEntry[] }) {
  const { t } = useI18n();
  return (
    <div className="log-list" role="log" aria-live="polite">
      {logs.length === 0 ? (
        <div className="log-empty">
          {t("暂无日志。启动连接后会在此显示本机事件。", "No log entries yet. Local events appear here after connecting.")}
        </div>
      ) : null}
      {logs.map((entry, index) => (
        <div className="log-line" key={`${entry.time}-${index}`}>
          <span className="log-time">{entry.time}</span>
          <span className="log-level" data-level={entry.level}>
            {entry.level}
          </span>
          <span>{entry.message}</span>
        </div>
      ))}
    </div>
  );
}
