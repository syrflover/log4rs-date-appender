use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::marker::PhantomData;
use std::path::PathBuf;

use log::Record;
use log4rs::append::Append;
use log4rs::encode::{self, Encode};
use parking_lot::Mutex;

use super::trigger::RollingTrigger;
use super::CurrentDate;

#[derive(Debug)]
pub struct LogWriter(BufWriter<File>);

impl From<BufWriter<File>> for LogWriter {
    fn from(writer: BufWriter<File>) -> Self {
        LogWriter(writer)
    }
}

impl io::Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl encode::Write for LogWriter {}

/// replace file name to
/// - {year} -> date.year
/// - {month} -> date.month
/// - {day} -> date.day
#[derive(Debug)]
pub struct DateRollingAppender<D>
where
    D: CurrentDate + 'static,
{
    writer: Mutex<Option<LogWriter>>,
    path: PathBuf,
    append: bool,
    encoder: Box<dyn Encode>,
    trigger: Box<dyn RollingTrigger>,
    _phantom: PhantomData<D>,
}

impl<D> DateRollingAppender<D>
where
    D: CurrentDate + 'static,
{
    pub fn builder() -> DateRollingAppenderBuilder<D> {
        DateRollingAppenderBuilder::<D> {
            append: None,
            encoder: None,
            trigger: None,
            path: None,
            _phantom: PhantomData,
        }
    }

    fn get_or_create_writer<'a>(
        &self,
        writer: &'a mut Option<LogWriter>,
    ) -> io::Result<&'a mut LogWriter> {
        match writer {
            Some(writer) => Ok(writer),
            None => {
                let mut path = self.path.clone();
                let formatted_file_name = {
                    let original_file_name = path.file_name().unwrap().to_str().unwrap();

                    let current_date = D::current_date();
                    let (year, month, day) = D::ymd(&current_date);

                    original_file_name
                        .replace("{year}", &year.to_string())
                        .replace("{month}", &month.to_string())
                        .replace("{day}", &day.to_string())
                };

                path.set_file_name(formatted_file_name);

                // 로그를 저장할 디렉토리가 없다면 생성함.
                if let Some(parent_path) = path.parent() {
                    if let Err(err) = fs::create_dir_all(parent_path) {
                        eprintln!("panicked at fs::create_dir_all(log_path): {err}");
                    }
                }

                let file = fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .append(self.append)
                    .truncate(!self.append)
                    .open(&path)?;

                let log_writer: LogWriter = BufWriter::with_capacity(1024, file).into();

                writer.replace(log_writer);

                Ok(writer.as_mut().unwrap())
            }
        }
    }
}

impl<D> Append for DateRollingAppender<D>
where
    D: CurrentDate + 'static,
{
    fn append(&self, record: &Record) -> anyhow::Result<()> {
        let mut writer = self.writer.lock();

        // 날짜가 지나면 기존 Writer를 제거하고,
        if self.trigger.trigger()? {
            *writer = None;
        }

        // writer가 None인 경우에는 현재 날짜 기준으로 writer를 생성함
        let writer = self.get_or_create_writer(&mut writer)?;
        self.encoder.encode(writer, record)?;
        writer.flush()?;

        Ok(())
    }

    fn flush(&self) {}
}

pub struct DateRollingAppenderBuilder<D>
where
    D: CurrentDate + 'static,
{
    path: Option<PathBuf>,
    encoder: Option<Box<dyn Encode>>,
    trigger: Option<Box<dyn RollingTrigger>>,
    append: Option<bool>,
    _phantom: PhantomData<D>,
}

impl<D> DateRollingAppenderBuilder<D>
where
    D: CurrentDate + 'static,
{
    pub fn path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.path.replace(path.into());

        self
    }

    pub fn encoder(mut self, encoder: Box<dyn Encode>) -> Self {
        self.encoder.replace(encoder);

        self
    }

    pub fn trigger(mut self, trigger: Box<dyn RollingTrigger>) -> Self {
        self.trigger.replace(trigger);

        self
    }

    pub fn append(mut self, append: bool) -> Self {
        self.append.replace(append);

        self
    }

    pub fn finalize(self) -> DateRollingAppender<D> {
        fn take<T>(mut opt: Option<T>, name: &str) -> T {
            let msg = format!("please set {}.", name);
            opt.take().expect(&msg)
        }

        DateRollingAppender::<D> {
            writer: Mutex::new(None),
            path: take(self.path, "DateRollingAppenderBuilder::path()"),
            encoder: take(self.encoder, "DateRollingAppenderBuilder::encoder()"),
            trigger: take(self.trigger, "DateRollingAppenderBuilder::trigger()"),
            append: take(self.append, "DateRollingAppenderBuilder::append()"),
            _phantom: PhantomData,
        }
    }
}
