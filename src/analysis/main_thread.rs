use crate::trace::{MainThreadSelection, TraceStore, event_store::is_task_name};

pub fn detect_main_thread(store: &TraceStore) -> MainThreadSelection {
    if let Some(t) = store
        .threads
        .iter()
        .filter(|t| t.thread_name.as_deref() == Some("CrRendererMain"))
        .max_by_key(|t| t.event_count)
    {
        return MainThreadSelection {
            pid: t.pid,
            tid: t.tid,
            confidence: "high".to_string(),
            explanation: format!(
                "Selected renderer main thread {:?} in process {:?} with {} events.",
                t.thread_name, t.process_name, t.event_count
            ),
        };
    }
    if let Some(t) = store
        .threads
        .iter()
        .filter(|t| {
            t.process_name.as_deref() == Some("Renderer")
                && t.thread_name.as_deref().is_some_and(|n| n.contains("Main"))
        })
        .max_by_key(|t| t.event_count)
    {
        return MainThreadSelection {
            pid: t.pid,
            tid: t.tid,
            confidence: "medium".to_string(),
            explanation: format!(
                "Selected renderer process main-like thread {:?} with {} events.",
                t.thread_name, t.event_count
            ),
        };
    }
    if let Some(t) = store
        .threads
        .iter()
        .filter(|t| t.thread_name.as_deref() == Some("CrBrowserMain"))
        .max_by_key(|t| t.event_count)
    {
        return MainThreadSelection {
            pid: t.pid,
            tid: t.tid,
            confidence: "medium".to_string(),
            explanation: format!(
                "No renderer main thread found; selected browser main thread {:?}.",
                t.thread_name
            ),
        };
    }
    if let Some(t) = store
        .threads
        .iter()
        .filter(|t| t.run_task_count > 0)
        .max_by(|a, b| {
            a.run_task_duration_us
                .partial_cmp(&b.run_task_duration_us)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    {
        return MainThreadSelection {
            pid: t.pid,
            tid: t.tid,
            confidence: if t.run_task_duration_us > 0.0 {
                "medium"
            } else {
                "low"
            }
            .to_string(),
            explanation: format!(
                "Selected thread with dominant {} task events totaling {:.2}ms.",
                t.run_task_count,
                t.run_task_duration_us / 1000.0
            ),
        };
    }
    if let Some(e) = store.events.iter().find(|e| is_task_name(&e.name)) {
        return MainThreadSelection {
            pid: e.pid,
            tid: e.tid,
            confidence: "low".to_string(),
            explanation: "Fallback selected first task event thread.".to_string(),
        };
    }
    let (pid, tid) = store
        .events
        .first()
        .map(|e| (e.pid, e.tid))
        .unwrap_or((0, 0));
    MainThreadSelection {
        pid,
        tid,
        confidence: "low".to_string(),
        explanation: "No task events found; selected first event thread.".to_string(),
    }
}
