/// 本次 GatewayController 调用对代理做了什么（供一键据实提示）。
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ProxyAction {
    Reused,    // 端口+adapter+key 指纹一致且健康，原样复用
    Restarted, // 首次起 / 换 key / 换 profile / 不健康，重起了代理
}

impl ProxyAction {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ProxyAction::Reused => "reused",
            ProxyAction::Restarted => "restarted",
        }
    }
}

/// 探活超时的原因措辞（纯函数，修真机 P2）：本地 `/health` 不验上游 key，故探活超时与 key 有效性
/// 无关。日志出现绑定失败（Address already in use / EADDRINUSE）→ 明确报端口占用；否则报「探活超时」
/// （多为 sidecar 缺失 / 启动异常），绝不再含糊说「或 key 无效」。
pub(crate) fn health_timeout_reason(port: u16, tail: &str) -> String {
    let occupied = tail.contains("Address already in use")
        || tail.contains("EADDRINUSE")
        || tail.contains("Errno 48") // macOS EADDRINUSE
        || tail.contains("Errno 98"); // Linux EADDRINUSE
    if occupied {
        format!("端口 {port} 已被占用，换个端口或先停掉占用进程后重试。")
    } else {
        format!(
            "代理起后探活超时（端口 {port}）：多为 Rust sidecar 缺失或启动异常，请查看代理日志。"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::health_timeout_reason;

    #[test]
    fn health_timeout_reason_flags_port_conflict_and_never_blames_key() {
        // 端口占用：明确报占用、带端口号，绝不提「key 无效」。
        let occ = health_timeout_reason(18991, "OSError: [Errno 48] Address already in use");
        assert!(occ.contains("18991"));
        assert!(occ.contains("占用"), "应明确报端口占用：{occ}");
        assert!(!occ.contains("key"), "端口占用不该扯上 key：{occ}");
        // 其它探活失败（依赖缺失等）：本地探活与 key 有效性无关，不得说「key 无效」。
        let generic = health_timeout_reason(18991, "failed to execute sidecar");
        assert!(
            !generic.contains("key 无效"),
            "本地探活超时与 key 有效性无关：{generic}"
        );
    }
}
