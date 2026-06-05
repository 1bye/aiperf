use crate::report::schema::Recommendation;

pub fn long_task_recommendation(files: Vec<String>) -> Recommendation {
    Recommendation { title: "Reduce dominant main-thread work inside long tasks".to_string(), why: "Long task breakdown shows blocking JS/React/layout work tied to evidence events.".to_string(), files_to_inspect: files, suggested_change: "Split work across frames, remove unnecessary renders/layout reads, and move parsing/normalization to a worker when CPU work dominates.".to_string(), risk: "medium".to_string() }
}
pub fn realtime_recommendation(files: Vec<String>) -> Recommendation {
    Recommendation { title: "Batch realtime updates before React/state fanout".to_string(), why: "Network burst evidence aligns with following long main-thread tasks.".to_string(), files_to_inspect: files, suggested_change: "Coalesce WebSocket/EventSource messages per animation frame, drop intermediate states, use store selectors, and defer heavy parse/normalize work to a Worker.".to_string(), risk: "medium".to_string() }
}
pub fn layout_recommendation(files: Vec<String>) -> Recommendation {
    Recommendation { title: "Remove forced layout/style work from interaction path".to_string(), why: "Style/layout evidence is nested near JS work and consumes meaningful task time.".to_string(), files_to_inspect: files, suggested_change: "Batch DOM reads before writes, reduce measured DOM size, virtualize large lists, and reduce IntersectionObserver targets.".to_string(), risk: "medium".to_string() }
}
