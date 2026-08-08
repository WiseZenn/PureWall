use windows::core::{Interface, HSTRING};
use windows::Win32::Foundation::GENERIC_READ;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppRGBA, IWICBitmapFrameDecode,
    IWICBitmapSourceTransform, IWICImagingFactory, IWICPalette, WICBitmapDitherTypeNone,
    WICBitmapInterpolationModeFant, WICBitmapPaletteTypeCustom, WICBitmapTransformRotate0,
    WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};

struct ComGuard(bool);

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

pub fn decode_target(source_path: &str, max_dimension: u32) -> Result<image::DynamicImage, String> {
    unsafe {
        let initialized = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let _guard = ComGuard(initialized);
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
                .map_err(|error| format!("create factory: {error}"))?;
        let decoder = factory
            .CreateDecoderFromFilename(
                &HSTRING::from(source_path),
                None,
                GENERIC_READ,
                WICDecodeMetadataCacheOnDemand,
            )
            .map_err(|error| format!("open source: {error}"))?;
        let frame = decoder
            .GetFrame(0)
            .map_err(|error| format!("read frame: {error}"))?;
        let mut width = 0;
        let mut height = 0;
        frame
            .GetSize(&mut width, &mut height)
            .map_err(|error| format!("read dimensions: {error}"))?;
        let (target_width, target_height) =
            crate::image_decoder::target_dimensions(width, height, max_dimension);

        let pixels = decode_with_source_transform(&frame, target_width, target_height)
            .or_else(|_| decode_with_scaler(&factory, &frame, target_width, target_height))?;
        let image = image::RgbaImage::from_raw(target_width, target_height, pixels)
            .ok_or_else(|| "WIC returned an invalid RGBA buffer".to_string())?;
        Ok(image::DynamicImage::ImageRgba8(image))
    }
}

unsafe fn decode_with_source_transform(
    frame: &IWICBitmapFrameDecode,
    target_width: u32,
    target_height: u32,
) -> Result<Vec<u8>, String> {
    let transform: IWICBitmapSourceTransform = frame
        .cast()
        .map_err(|error| format!("source transform unavailable: {error}"))?;
    let mut width = target_width;
    let mut height = target_height;
    transform
        .GetClosestSize(&mut width, &mut height)
        .map_err(|error| format!("closest size: {error}"))?;
    if width != target_width || height != target_height {
        return Err("decoder cannot produce the exact target size".to_string());
    }
    let mut pixel_format = GUID_WICPixelFormat32bppRGBA;
    transform
        .GetClosestPixelFormat(&mut pixel_format)
        .map_err(|error| format!("closest pixel format: {error}"))?;
    if pixel_format != GUID_WICPixelFormat32bppRGBA {
        return Err("decoder cannot directly produce RGBA pixels".to_string());
    }

    let (stride, mut pixels) = rgba_buffer(width, height)?;
    transform
        .CopyPixels(
            std::ptr::null(),
            width,
            height,
            &pixel_format,
            WICBitmapTransformRotate0,
            stride,
            &mut pixels,
        )
        .map_err(|error| format!("source transform copy: {error}"))?;
    Ok(pixels)
}

unsafe fn decode_with_scaler(
    factory: &IWICImagingFactory,
    frame: &IWICBitmapFrameDecode,
    target_width: u32,
    target_height: u32,
) -> Result<Vec<u8>, String> {
    let scaler = factory
        .CreateBitmapScaler()
        .map_err(|error| format!("create scaler: {error}"))?;
    scaler
        .Initialize(
            frame,
            target_width,
            target_height,
            WICBitmapInterpolationModeFant,
        )
        .map_err(|error| format!("scale frame: {error}"))?;
    let converter = factory
        .CreateFormatConverter()
        .map_err(|error| format!("create converter: {error}"))?;
    converter
        .Initialize(
            &scaler,
            &GUID_WICPixelFormat32bppRGBA,
            WICBitmapDitherTypeNone,
            None::<&IWICPalette>,
            0.0,
            WICBitmapPaletteTypeCustom,
        )
        .map_err(|error| format!("convert RGBA: {error}"))?;
    let (stride, mut pixels) = rgba_buffer(target_width, target_height)?;
    converter
        .CopyPixels(std::ptr::null(), stride, &mut pixels)
        .map_err(|error| format!("copy scaled pixels: {error}"))?;
    Ok(pixels)
}

fn rgba_buffer(width: u32, height: u32) -> Result<(u32, Vec<u8>), String> {
    let stride = width
        .checked_mul(4)
        .ok_or_else(|| "WIC target stride overflow".to_string())?;
    let length = stride
        .checked_mul(height)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| "WIC target buffer overflow".to_string())?;
    Ok((stride, vec![0; length]))
}
