mod appender;
mod trigger;

use std::fmt::Debug;
use std::path::Path;

use chrono::{Datelike, NaiveDateTime, Utc};
use log::LevelFilter;
use log4rs::append::console::ConsoleAppender;
use log4rs::append::Append;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::encode::Encode;

use self::appender::DateRollingAppender;
use self::trigger::{DateTrigger, RollingTrigger};

pub trait CurrentDate: Debug + Send + Sync {
    fn current_date() -> NaiveDateTime {
        Utc::now().naive_local()
    }

    /// Year, Month, Day
    fn ymd(date: &NaiveDateTime) -> (i32, u32, u32) {
        (date.year(), date.month(), date.day())
    }
}

// const TAB: &str = "    ";

fn encoder() -> Box<impl Encode> {
    // `[2021-08-27 21:56:44 +09:00] - [INFO] Message`
    // let encoder = PatternEncoder::new("[{d(%Y-%m-%d %H:%M:%S %Z)}] - [{l}] {m}{n}");

    // [2021-08-30 19:09:42 +09:00] - [INFO] MODULE = df_user::adapter::controller::websocket, PID = 53280, THREAD = tokio-runtime-worker-123145352740864
    //     started WebSocketServer
    let encoder = PatternEncoder::new(
        "[{date(%Y-%m-%d %H:%M:%S %Z)}] - [{level}] MODULE = {module}, PID = {pid}, THREAD = {thread}-{I}{n}{message}{n}", // .replace("{tab}", TAB),
    );
    Box::new(encoder)
}

fn date_trigger<D>() -> Box<impl RollingTrigger>
where
    D: CurrentDate + 'static,
{
    let date_trigger = DateTrigger::<D>::new();
    Box::new(date_trigger)
}

/// replace file name to
/// - {year} -> date.year
/// - {month} -> date.month
/// - {day} -> date.day
fn date_appender<D>(path: impl AsRef<Path>) -> Box<impl Append>
where
    D: CurrentDate + 'static,
{
    let date_appender = DateRollingAppender::<D>::builder()
        .append(true)
        .encoder(encoder())
        .trigger(date_trigger::<D>())
        .path(path.as_ref())
        .finalize();
    Box::new(date_appender)
}

/* fn stderr() -> ConsoleAppender {
    ConsoleAppender::builder()
        .encoder(encoder())
        .target(Target::Stderr)
        .build()
} */

fn console() -> Box<impl Append> {
    let console = ConsoleAppender::builder().encoder(encoder()).build();
    Box::new(console)
}

/* fn env_level() -> LevelFilter {
    use LevelFilter::*;

    let level = std::env::var("LOG_LEVEL")
        .unwrap_or_else(|_| "info".into())
        .to_lowercase();

    match level.as_str() {
        "error" => Error,
        "warn" => Warn,
        "info" => Info,
        "debug" => Debug,
        "trace" => Trace,
        _ => panic!("wrong LOG_LEVEL"),
    }
} */

/// path: log/{year}-{month}-{day}.log
pub fn config<D>(p: impl AsRef<Path> + 'static, level: LevelFilter) -> log4rs::Config
where
    D: CurrentDate + 'static,
{
    // let level = env_level();

    let stdout_console = Appender::builder().build("stdout_console", console());
    let stdout_file = Appender::builder().build("stdout_file", date_appender::<D>(p));

    /* if LevelFilter::Warn <= level {
        LevelFilter::Warn
    } else {
        level
    } */

    // allow unused_mut, for test
    #[allow(unused_mut)]
    let mut logger_builder = log4rs::Config::builder().appender(stdout_console);

    #[cfg(test)]
    {
        use log4rs::config::Logger;

        let test = Appender::builder().build(
            "test",
            date_appender::<D>(".temp/test-{year}-{month}-{day}.log"),
        );

        let test_logger = Logger::builder()
            .additive(false)
            .appender("test")
            .build("log4rs_date_appender", level);

        // wrap in if expression block, because unreachable_code warning occured in rust-analyzer
        if true {
            let root = Root::builder().appender("stdout_console").build(level);

            return logger_builder
                .appender(test)
                .logger(test_logger)
                .build(root)
                .unwrap();
        }
    }

    let root = Root::builder()
        .appender("stdout_file")
        .appender("stdout_console")
        .build(level);

    logger_builder.appender(stdout_file).build(root).unwrap()
}

pub fn init_config(config: log4rs::Config) -> log4rs::Handle {
    log4rs::init_config(config).expect("can't initialize log4rs")
}

#[cfg(test)]
mod tests {
    use std::{fs, io};

    use chrono::{NaiveDateTime, Utc};
    use fake::Fake;

    use super::{config, CurrentDate};

    static mut ADD: i64 = 0;

    #[derive(Debug)]
    struct TestCurrentDate;

    impl CurrentDate for TestCurrentDate {
        fn current_date() -> NaiveDateTime {
            Utc::now().naive_local() - chrono::Duration::days(1 - unsafe { ADD })
        }
    }

    #[test]
    fn changed_day() {
        match fs::read_dir(".temp") {
            Ok(logs) => {
                // 이전 테스트 결과물 삭제
                for log in logs {
                    let log = log.unwrap();
                    let log = log.file_name();
                    let log = log.to_str().unwrap();

                    if log.starts_with("test-") && log.ends_with(".log") {
                        fs::remove_file(format!(".temp/{}", log)).unwrap();
                    }
                }
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => panic!("{:?}", err),
        }

        log4rs::init_config(config::<TestCurrentDate>(".temp", log::LevelFilter::Trace)).unwrap();

        let yesterday = TestCurrentDate::current_date();

        log::info!("1. {}", fake::faker::lorem::en::Word().fake::<String>());
        log::info!("2. {}", fake::faker::lorem::en::Word().fake::<String>());
        log::info!("3. {}", fake::faker::lorem::en::Word().fake::<String>());

        unsafe {
            // 하루가 지났음.
            ADD += 1;
        }

        let today = TestCurrentDate::current_date();

        log::info!("4. {}", fake::faker::lorem::en::Word().fake::<String>());
        log::info!("5. {}", fake::faker::lorem::en::Word().fake::<String>());
        log::info!("6. {}", fake::faker::lorem::en::Word().fake::<String>());

        // sleep(Duration::from_millis(500)).await;

        let yesterday_logs = {
            let (year, month, day) = TestCurrentDate::ymd(&yesterday);
            let path = format!(".temp/test-{year}-{month}-{day}.log");
            fs::read_to_string(path).unwrap()
        };

        let today_logs = {
            let (year, month, day) = TestCurrentDate::ymd(&today);
            let path = format!(".temp/test-{year}-{month}-{day}.log");
            fs::read_to_string(path).unwrap()
        };

        for (i, log) in yesterday_logs.lines().enumerate() {
            // 로그 한 번에 두 줄을 사용하기 때문에
            if i + 1 == 0 {
                // 1,2,3 비교
                assert!(log.contains(&format!("{}.", i + 1)));
            }
        }

        for (i, log) in today_logs.lines().enumerate() {
            // 로그 한 번에 두 줄을 사용하기 때문에
            if i + 1 == 0 {
                // 4,5,6 비교
                assert!(log.contains(&format!("{}.", i + 4)));
            }
        }
    }
}
