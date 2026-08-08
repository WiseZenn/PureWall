use serde::Serialize;

#[derive(Clone, Default, Serialize)]
pub struct ShellMetadata {
    pub authors: String,
    pub copyright: String,
    pub comment: String,
}

#[cfg(windows)]
pub fn read(path: &str) -> Result<ShellMetadata, String> {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::System::Com::StructuredStorage::PropVariantClear;
    use windows::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };
    use windows::Win32::UI::Shell::PropertiesSystem::{
        IPropertyStore, PSFormatForDisplay, PSGetPropertyKeyFromName,
        SHGetPropertyStoreFromParsingName, GPS_DEFAULT, PDFF_DEFAULT,
    };

    unsafe fn formatted_property(store: &IPropertyStore, name: &str) -> String {
        let mut key = PROPERTYKEY::default();
        if PSGetPropertyKeyFromName(&HSTRING::from(name), &mut key).is_err() {
            return String::new();
        }

        let Ok(mut value) = store.GetValue(&key) else {
            return String::new();
        };

        let mut buffer = [0u16; 1024];
        let result = if PSFormatForDisplay(&key, &value, PDFF_DEFAULT, &mut buffer).is_ok() {
            let null_pos = buffer.iter().position(|v| *v == 0);
            let (length, truncated) = match null_pos {
                Some(pos) => (pos, false),
                None => (buffer.len(), true),
            };
            let mut text = String::from_utf16_lossy(&buffer[..length])
                .trim()
                .to_string();
            if truncated {
                text.push('\u{2026}'); // ellipsis
            }
            text
        } else {
            String::new()
        };
        let _ = PropVariantClear(&mut value);
        result
    }

    unsafe {
        let initialized =
            CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).is_ok();

        let result = (|| {
            let store: IPropertyStore =
                SHGetPropertyStoreFromParsingName(&HSTRING::from(path), None, GPS_DEFAULT)
                    .map_err(|error| error.to_string())?;

            Ok(ShellMetadata {
                authors: formatted_property(&store, "System.Author"),
                copyright: formatted_property(&store, "System.Copyright"),
                comment: formatted_property(&store, "System.Comment"),
            })
        })();

        if initialized {
            CoUninitialize();
        }
        result
    }
}

#[cfg(windows)]
pub fn write_title(path: &str, title: &str) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::System::Com::StructuredStorage::{
        InitPropVariantFromStringAsVector, PropVariantClear,
    };
    use windows::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };
    use windows::Win32::UI::Shell::PropertiesSystem::{
        IPropertyStore, PSCoerceToCanonicalValue, PSGetPropertyKeyFromName,
        SHGetPropertyStoreFromParsingName, GPS_READWRITE,
    };

    let title = title.trim();

    unsafe {
        let initialized =
            CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).is_ok();

        let result = (|| {
            let store: IPropertyStore =
                SHGetPropertyStoreFromParsingName(&HSTRING::from(path), None, GPS_READWRITE)
                    .map_err(|e| format!("Failed to open property store: {e}"))?;

            let mut key = PROPERTYKEY::default();
            PSGetPropertyKeyFromName(&HSTRING::from("System.Title"), &mut key)
                .map_err(|e| format!("Failed to resolve System.Title: {e}"))?;

            if title.is_empty() {
                // Clear the property by setting an empty value.
                store
                    .SetValue(&key, &Default::default())
                    .map_err(|e| format!("Failed to clear title: {e}"))?;
            } else {
                let mut propvar = InitPropVariantFromStringAsVector(&HSTRING::from(title))
                    .map_err(|e| format!("Failed to create property value: {e}"))?;
                PSCoerceToCanonicalValue(&key, &mut propvar)
                    .map_err(|e| format!("Failed to canonicalize System.Title: {e}"))?;
                let set_result = store.SetValue(&key, &propvar);
                let clear_result = PropVariantClear(&mut propvar);
                set_result.map_err(|e| format!("Failed to set title: {e}"))?;
                clear_result.map_err(|e| format!("Failed to clear title value: {e}"))?;
            }

            store
                .Commit()
                .map_err(|e| format!("Failed to commit title to file: {e}"))?;
            Ok(())
        })();

        if initialized {
            CoUninitialize();
        }
        result
    }
}

#[cfg(not(windows))]
pub fn write_title(_path: &str, _title: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn read(_path: &str) -> Result<ShellMetadata, String> {
    Ok(ShellMetadata::default())
}

#[cfg(all(test, windows))]
mod tests {
    use super::write_title;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempJpeg(PathBuf);

    impl TempJpeg {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should follow Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "purewall-shell-title-{}-{unique}.jpg",
                std::process::id()
            ));
            let image = image::RgbImage::from_pixel(4, 4, image::Rgb([18, 42, 64]));
            image
                .save_with_format(&path, image::ImageFormat::Jpeg)
                .expect("temporary JPEG should be created");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempJpeg {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn write_title_opens_a_writable_jpeg_property_store() {
        let file = TempJpeg::new();
        let path = file
            .path()
            .to_str()
            .expect("temporary JPEG path should be Unicode");
        let result = write_title(path, "Road Lights");

        assert!(
            result.is_ok(),
            "System.Title should be committed to a writable JPEG property store: {result:?}"
        );
    }
}
