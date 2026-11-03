// Stage one deliberately leaves macOS unsupported. Keep the platform contract
// identical so ScreenCaptureKit can be implemented and tested on a Mac later.
use super::{RecordOptions, RecordResult, RecordStatus};
use std::path::Path;

pub struct Recorder;
pub fn supported() -> bool { false }
fn unsupported<T>() -> Result<T, String> { Err("screen_record.unsupported".into()) }
impl Recorder {
    pub fn start(_: RecordOptions) -> Result<Self, String> { unsupported() }
    pub fn status(&self) -> RecordStatus { unreachable!("unsupported platform cannot start a recorder") }
    pub fn pause(&mut self) -> Result<(), String> { unsupported() }
    pub fn resume(&mut self) -> Result<(), String> { unsupported() }
    pub fn stop(&mut self) -> Result<RecordResult, String> { unsupported() }
    pub fn cancel(&mut self) -> Result<(), String> { unsupported() }
}
pub fn copy_file(_: &Path) -> Result<(), String> { unsupported() }
pub fn reveal(_: &Path) -> Result<(), String> { unsupported() }
