use crate::report::schema::Breakdown;
use crate::trace::{Category, TraceEvent};

pub fn add_event_to_breakdown(b: &mut Breakdown, e: &TraceEvent) {
    let ms = e.dur_ms();
    match e.category {
        Category::Js
        | Category::Timers
        | Category::AnimationFrame
        | Category::Input
        | Category::Scroll => b.js_ms += ms,
        Category::React => b.react_ms += ms,
        Category::Style => b.style_ms += ms,
        Category::Layout => b.layout_ms += ms,
        Category::Paint | Category::Composite | Category::Raster | Category::Gpu => {
            b.paint_composite_ms += ms
        }
        Category::Gc => b.gc_ms += ms,
        Category::ParseCompile => b.parse_compile_ms += ms,
        Category::Network | Category::Idle | Category::HitTest | Category::Unknown => {
            b.unknown_ms += ms
        }
    }
}

pub fn categorized_ms(b: &Breakdown) -> f64 {
    b.js_ms
        + b.react_ms
        + b.style_ms
        + b.layout_ms
        + b.paint_composite_ms
        + b.gc_ms
        + b.parse_compile_ms
}
