//! Quota windows, cached state, and app-server response parsing.
use serde_json::Value;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Window {
    pub remaining: f64,
    pub minutes: u64,
    pub resets_at: Option<u64>,
}

impl Window {
    pub(crate) fn label(&self) -> String {
        match self.minutes {
            10080 => "1W".into(),
            n if n % 60 == 0 => format!("{}H", n / 60),
            n => format!("{}m", n),
        }
    }

    pub(crate) fn description(&self) -> String {
        match self.minutes {
            10080 => "每周额度".into(),
            n if n % 60 == 0 => format!("{} 小时额度", n / 60),
            n => format!("{} 分钟额度", n),
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct State {
    pub windows: [Option<Window>; 2],
    pub updated: Option<Instant>,
    pub error: Option<String>,
}

impl State {
    pub(crate) fn stale(&self) -> bool {
        self.error.is_some()
            || self
                .updated
                .is_none_or(|t| t.elapsed() > Duration::from_secs(120))
    }
}

pub(super) fn parse(value: &Value) -> Result<[Option<Window>; 2], String> {
    // A multi-bucket response is authoritative: never display some other quota as Codex.
    let bucket = if let Some(buckets) = value.get("rateLimitsByLimitId").filter(|v| !v.is_null()) {
        buckets.get("codex")
    } else {
        value.get("rateLimits").filter(|b| {
            b.get("limitId")
                .and_then(Value::as_str)
                .is_none_or(|id| id == "codex")
        })
    }
    .ok_or("账户没有返回 Codex 额度")?;
    let window = |key: &str| -> Option<Window> {
        let v = bucket.get(key)?;
        let used = v.get("usedPercent")?.as_f64()?;
        let minutes = v.get("windowDurationMins")?.as_u64()?;
        if !used.is_finite() || minutes == 0 {
            return None;
        }
        Some(Window {
            remaining: (100.0 - used).clamp(0.0, 100.0),
            minutes,
            resets_at: v.get("resetsAt").and_then(Value::as_u64),
        })
    };
    let windows = [window("primary"), window("secondary")];
    if windows.iter().all(Option::is_none) {
        return Err("暂无可用的额度数据".into());
    }
    Ok(windows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn prefers_codex_bucket_and_keeps_unknown_window_unknown() {
        let v = json!({"rateLimitsByLimitId":{"codex":{"primary":{"usedPercent":37,"windowDurationMins":300}}},
            "rateLimits":{"primary":{"usedPercent":99,"windowDurationMins":300}}});
        let windows = parse(&v).unwrap();
        assert_eq!(windows[0].as_ref().unwrap().remaining, 63.0);
        assert!(windows[1].is_none());
        assert!(parse(&json!({"rateLimitsByLimitId":{"other":{}}})).is_err());
    }
    #[test]
    fn supports_legacy_and_clamps_percentages() {
        let windows = parse(
            &json!({"rateLimits":{"primary":{"usedPercent":105,"windowDurationMins":300},
            "secondary":{"usedPercent":-2,"windowDurationMins":10080}}}),
        )
        .unwrap();
        assert_eq!(windows[0].as_ref().unwrap().remaining, 0.0);
        assert_eq!(windows[1].as_ref().unwrap().remaining, 100.0);
        assert_eq!(windows[1].as_ref().unwrap().label(), "1W");
        assert!(parse(&json!({"rateLimits":null})).is_err());
    }
}
