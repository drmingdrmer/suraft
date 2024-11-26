use std::backtrace::Backtrace;
use std::env;
use std::panic::PanicHookInfo;

use tracing::Event;
use tracing::Subscriber;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::RollingFileAppender;
use tracing_appender::rolling::Rotation;
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::fmt::time::SystemTime;
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::fmt::FormatEvent;
use tracing_subscriber::fmt::FormatFields;
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Registry;

pub fn init_logging(app_name: &str, dir: &str, level: &str) -> WorkerGuard {
    set_panic_hook();

    let (g, sub) = init_file_logging(app_name, dir, level);
    tracing::subscriber::set_global_default(sub)
        .expect("error setting global tracing subscriber");

    tracing::info!(
        "initialized global tracing: in {}/{} at {}",
        dir,
        app_name,
        level
    );
    g
}
pub fn set_panic_hook() {
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        log_panic(panic);
        prev_hook(panic);
    }));
}
pub fn log_panic(panic: &PanicHookInfo) {
    let backtrace = { format!("{:?}", Backtrace::force_capture()) };

    if let Some(location) = panic.location() {
        tracing::error!(
            message = %panic.to_string().replace('\n', " "),
            backtrace = %backtrace,
            panic.file = location.file(),
            panic.line = location.line(),
            panic.column = location.column(),
        );
    } else {
        tracing::error!(message = %panic.to_string().replace('\n', " "), backtrace = %backtrace);
    }
}

pub fn init_file_logging(
    app_name: &str,
    dir: &str,
    level: &str,
) -> (WorkerGuard, impl Subscriber) {
    // open log file

    let f = RollingFileAppender::new(Rotation::HOURLY, dir, app_name);

    // build subscriber

    let (writer, writer_guard) = tracing_appender::non_blocking(f);

    let f_layer = fmt::Layer::new()
        .with_span_events(fmt::format::FmtSpan::NONE)
        .with_writer(writer)
        .with_ansi(false)
        .event_format(EventFormatter {});

    // Use env RUST_LOG to initialize log if present.
    // Otherwise, use the specified level.
    let directives =
        env::var(EnvFilter::DEFAULT_ENV).unwrap_or_else(|_x| level.to_string());
    let env_filter = EnvFilter::new(directives);

    let subscriber = Registry::default().with(env_filter).with(f_layer);

    (writer_guard, subscriber)
}

pub struct EventFormatter {}

impl<S, N> FormatEvent<S, N> for EventFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let meta = event.metadata();

        SystemTime {}.format_time(&mut writer)?;
        writer.write_char(' ')?;

        let fmt_level = meta.level().as_str();
        write!(writer, "{:>5} ", fmt_level)?;

        ctx.format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}
