use super::model::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStore {
    pub metadata: TraceMetadata,
    pub events: Vec<TraceEvent>,
    pub origin_ts_us: f64,
    pub end_ts_us: f64,
    pub threads: Vec<ThreadInfo>,
    #[serde(skip)]
    pub by_name: HashMap<String, Vec<EventId>>,
    #[serde(skip)]
    pub by_thread: HashMap<(u64, u64), Vec<EventId>>,
    #[serde(skip)]
    pub by_category: HashMap<Category, Vec<EventId>>,
    #[serde(skip)]
    pub children: HashMap<EventId, Vec<EventId>>,
}

impl TraceStore {
    pub fn new(
        metadata: TraceMetadata,
        mut events: Vec<TraceEvent>,
        process_names: BTreeMap<u64, String>,
        thread_names: BTreeMap<(u64, u64), String>,
    ) -> Self {
        events.sort_by(|a, b| {
            a.ts_us
                .partial_cmp(&b.ts_us)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.event_id.cmp(&b.event_id))
        });
        assign_parent_ids(&mut events);
        let range_events: Vec<&TraceEvent> = events.iter().filter(|e| is_timing_event(e)).collect();
        let range_source: Vec<&TraceEvent> = if range_events.is_empty() {
            events.iter().collect()
        } else {
            range_events
        };
        let origin_ts_us = range_source
            .iter()
            .map(|e| e.ts_us)
            .fold(f64::INFINITY, f64::min);
        let end_ts_us = range_source
            .iter()
            .map(|e| e.end_us.max(e.ts_us))
            .fold(0.0_f64, f64::max);
        let origin_ts_us = if origin_ts_us.is_finite() {
            origin_ts_us
        } else {
            0.0
        };
        let mut store = Self {
            metadata,
            events,
            origin_ts_us,
            end_ts_us,
            threads: Vec::new(),
            by_name: HashMap::new(),
            by_thread: HashMap::new(),
            by_category: HashMap::new(),
            children: HashMap::new(),
        };
        store.rebuild_indexes(process_names, thread_names);
        store
    }

    pub fn rebuild_indexes(
        &mut self,
        process_names: BTreeMap<u64, String>,
        thread_names: BTreeMap<(u64, u64), String>,
    ) {
        self.by_name.clear();
        self.by_thread.clear();
        self.by_category.clear();
        self.children.clear();
        let mut thread_map: BTreeMap<(u64, u64), ThreadInfo> = BTreeMap::new();
        for e in &self.events {
            self.by_name
                .entry(e.name.clone())
                .or_default()
                .push(e.event_id);
            self.by_thread
                .entry((e.pid, e.tid))
                .or_default()
                .push(e.event_id);
            self.by_category
                .entry(e.category)
                .or_default()
                .push(e.event_id);
            if let Some(parent_id) = e.parent_id {
                self.children.entry(parent_id).or_default().push(e.event_id);
            }
            let ti = thread_map
                .entry((e.pid, e.tid))
                .or_insert_with(|| ThreadInfo {
                    pid: e.pid,
                    tid: e.tid,
                    thread_name: thread_names.get(&(e.pid, e.tid)).cloned(),
                    process_name: process_names.get(&e.pid).cloned(),
                    event_count: 0,
                    run_task_count: 0,
                    run_task_duration_us: 0.0,
                });
            ti.event_count += 1;
            if is_task_name(&e.name) && e.is_complete() {
                ti.run_task_count += 1;
                ti.run_task_duration_us += e.dur_us;
            }
        }
        self.threads = thread_map.into_values().collect();
    }

    pub fn duration_us(&self) -> f64 {
        (self.end_ts_us - self.origin_ts_us).max(0.0)
    }
    pub fn duration_ms(&self) -> f64 {
        self.duration_us() / 1000.0
    }

    pub fn event(&self, id: EventId) -> Option<&TraceEvent> {
        self.events.iter().find(|e| e.event_id == id)
    }

    pub fn events_in_window(&self, start_us: f64, end_us: f64) -> Vec<&TraceEvent> {
        self.events
            .iter()
            .filter(|e| e.ts_us < end_us && e.end_us > start_us)
            .collect()
    }

    pub fn child_events(&self, id: EventId) -> Vec<&TraceEvent> {
        self.children
            .get(&id)
            .into_iter()
            .flatten()
            .filter_map(|cid| self.event(*cid))
            .collect()
    }

    pub fn thread_events(&self, pid: u64, tid: u64) -> Vec<&TraceEvent> {
        self.by_thread
            .get(&(pid, tid))
            .into_iter()
            .flatten()
            .filter_map(|id| self.event(*id))
            .collect()
    }

    pub fn unique_pids(&self) -> Vec<u64> {
        let set: BTreeSet<u64> = self.events.iter().map(|e| e.pid).collect();
        set.into_iter().collect()
    }
}

pub fn is_task_name(name: &str) -> bool {
    matches!(name, "RunTask" | "ThreadControllerImpl::RunTask" | "Task")
}

fn is_timing_event(e: &TraceEvent) -> bool {
    e.pid != 0 && e.phase != "M" && !e.name.ends_with("_name")
}

fn assign_parent_ids(events: &mut [TraceEvent]) {
    let mut stacks: HashMap<(u64, u64), Vec<(EventId, f64)>> = HashMap::new();
    for e in events.iter_mut() {
        if !e.is_complete() {
            continue;
        }
        let stack = stacks.entry((e.pid, e.tid)).or_default();
        while stack.last().is_some_and(|(_, end)| *end <= e.ts_us) {
            stack.pop();
        }
        e.parent_id = stack.last().map(|(id, _)| *id);
        stack.push((e.event_id, e.end_us));
    }
}
