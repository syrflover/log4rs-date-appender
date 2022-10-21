use std::fmt::Debug;
use std::marker::PhantomData;

use chrono::Datelike;
use parking_lot::Mutex;

use super::CurrentDate;

pub trait RollingTrigger: Send + Sync + Debug {
    fn trigger(&self) -> anyhow::Result<bool>;
}

#[derive(Debug)]
pub struct DateTrigger<D>
where
    D: CurrentDate + 'static,
{
    inner: Mutex<u32>,
    _phantom: PhantomData<D>,
}

impl<D> DateTrigger<D>
where
    D: CurrentDate + 'static,
{
    pub fn new() -> Self {
        let current_day = D::current_date().day();
        Self {
            inner: Mutex::new(current_day),
            _phantom: PhantomData,
        }
    }
}

impl<D> RollingTrigger for DateTrigger<D>
where
    D: CurrentDate + 'static,
{
    fn trigger(&self) -> anyhow::Result<bool> {
        let mut last_updated_at = self.inner.lock();
        let current_day = D::current_date().day();

        let updated = current_day != *last_updated_at;

        if updated {
            *last_updated_at = current_day;
        }

        Ok(updated)
    }
}
