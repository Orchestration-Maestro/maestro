//! Image media behaviour through the owned loader.
use maestro_terminal::kitty::graphics::{command::*, image::*};
#[cfg(target_os = "linux")]
use std::{fs::File, sync::atomic::Ordering};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

include!("support/images.rs");

fn command(data: &[u8]) -> Command {
    Command {
        control: Control::Transmit(Transmission {
            format: Format::Rgb,
            width: 1,
            height: 1,
            ..Transmission::default()
        }),
        quiet: Quiet::No,
        data: data.to_vec(),
    }
}

fn transmission_mut(cmd: &mut Command) -> &mut Transmission {
    match &mut cmd.control {
        Control::Transmit(t) => t,
        _ => panic!("expected transmission"),
    }
}

fn local(path: &Path, medium: Medium) -> Command {
    let mut cmd = command(path.as_os_str().as_encoded_bytes());
    transmission_mut(&mut cmd).medium = medium;
    cmd
}

fn complete(cmd: &Command) -> Result<Image, ImageError> {
    LoadingImage::init(cmd, Limits::ALL)?.complete()
}

fn assert_error(cmd: &Command, expected: ImageError) {
    assert_eq!(complete(cmd), Err(expected));
}

#[cfg(target_os = "linux")]
/// Per-process shared-memory segment identity allocation.
static NEXT_FILE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
#[cfg(target_os = "linux")]
struct Segment(String);
#[cfg(target_os = "linux")]
impl Segment {
    fn new(data: &[u8]) -> Self {
        use rustix::shm::{self, Mode, OFlags};
        let name = format!(
            "/maestro-image-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        );
        let fd = shm::open(
            name.as_str(),
            OFlags::CREATE | OFlags::EXCL | OFlags::RDWR,
            Mode::RUSR | Mode::WUSR,
        )
        .unwrap();
        let mut file = File::from(fd);
        file.write_all(data).unwrap();
        Self(name)
    }
    fn command(&self) -> Command {
        let mut cmd = command(self.0.as_bytes());
        transmission_mut(&mut cmd).medium = Medium::SharedMemory;
        cmd
    }
    fn exists(&self) -> bool {
        rustix::shm::open(
            self.0.as_str(),
            rustix::shm::OFlags::RDONLY,
            rustix::shm::Mode::empty(),
        )
        .is_ok()
    }
}
#[cfg(target_os = "linux")]
impl Drop for Segment {
    fn drop(&mut self) {
        let _ = rustix::shm::unlink(self.0.as_str());
    }
}

#[test]
fn initial_load_defers_raw_length_validation() {
    let loading = LoadingImage::init(&command(b"AAAA"), Limits::DIRECT).unwrap();
    assert_eq!(loading.data, b"AAAA");
    assert_eq!(loading.image.width, 1);
    assert_eq!(loading.image.height, 1);
}

#[test]
fn completion_rejects_excessive_width() {
    let mut cmd = command(b"AAAA");
    transmission_mut(&mut cmd).width = 10001;
    assert_error(&cmd, ImageError::DimensionsTooLarge);
}

#[test]
fn completion_rejects_excessive_height() {
    let mut cmd = command(b"AAAA");
    transmission_mut(&mut cmd).height = 10001;
    assert_error(&cmd, ImageError::DimensionsTooLarge);
}

#[test]
fn direct_zlib_pixels_are_inflated() {
    let mut cmd = command(include_bytes!("fixtures/images/pixels128x96.zlib"));
    let t = transmission_mut(&mut cmd);
    t.width = 128;
    t.height = 96;
    t.compression = Compression::ZlibDeflate;
    let image = complete(&cmd).unwrap();
    assert_eq!(
        (image.width, image.height, image.format, image.compression),
        (128, 96, Format::Rgb, Compression::None)
    );
    assert_eq!(
        image.data,
        include_bytes!("fixtures/images/pixels128x96.rgb")
    );
}

#[test]
fn direct_pixels_are_retained() {
    let pixels = include_bytes!("fixtures/images/pixels20x15.rgb");
    let mut cmd = command(pixels);
    let t = transmission_mut(&mut cmd);
    t.width = 20;
    t.height = 15;
    let image = complete(&cmd).unwrap();
    assert_eq!(
        (image.width, image.height, image.compression),
        (20, 15, Compression::None)
    );
    assert_eq!(image.data, pixels);
}

fn chunked(first: usize) -> Image {
    let bytes = include_bytes!("fixtures/images/pixels128x96.zlib");
    let mut cmd = command(&bytes[..first]);
    let t = transmission_mut(&mut cmd);
    t.width = 128;
    t.height = 96;
    t.compression = Compression::ZlibDeflate;
    t.more_chunks = true;
    let mut loading = LoadingImage::init(&cmd, Limits::DIRECT).unwrap();
    for chunk in bytes[first..].chunks(1024) {
        loading.add_data(chunk).unwrap();
    }
    loading.complete().unwrap()
}

#[test]
fn compressed_chunks_form_one_image() {
    let image = chunked(1024);
    assert_eq!(
        (image.width, image.height, image.format, image.compression),
        (128, 96, Format::Rgb, Compression::None)
    );
    assert_eq!(
        image.data,
        include_bytes!("fixtures/images/pixels128x96.rgb")
    );
}

#[test]
fn empty_first_chunk_keeps_image_metadata() {
    let image = chunked(0);
    assert_eq!(
        (image.width, image.height, image.format, image.compression),
        (128, 96, Format::Rgb, Compression::None)
    );
    assert_eq!(
        image.data,
        include_bytes!("fixtures/images/pixels128x96.rgb")
    );
}

#[test]
fn completion_validates_dimensions_and_raw_length() {
    for (width, height) in [(0, 1), (1, 0)] {
        let mut cmd = command(b"");
        let t = transmission_mut(&mut cmd);
        t.width = width;
        t.height = height;
        assert_error(&cmd, ImageError::DimensionsRequired);
    }
    for data in [b"AB".as_slice(), b"ABCD".as_slice()] {
        assert_error(&command(data), ImageError::InvalidData);
    }
    for (format, bpp) in [
        (Format::Gray, 1),
        (Format::GrayAlpha, 2),
        (Format::Rgb, 3),
        (Format::Rgba, 4),
    ] {
        assert_eq!(Transmission::format_bpp(format), Some(bpp));
        let mut cmd = command(&b"ABCD"[..usize::from(bpp)]);
        transmission_mut(&mut cmd).format = format;
        assert_eq!(complete(&cmd).unwrap().data, &b"ABCD"[..usize::from(bpp)]);
    }
    assert_eq!(Transmission::format_bpp(Format::Png), None);
    for (width, height) in [(10000, 1), (1, 10000)] {
        let mut cmd = command(&vec![b'A'; 30000]);
        let t = transmission_mut(&mut cmd);
        t.width = width;
        t.height = height;
        let image = complete(&cmd).unwrap();
        assert_eq!((image.width, image.height), (width, height));
        assert_eq!(image.data, vec![b'A'; 30000]);
    }
}

#[test]
fn zlib_failure_and_tail_rules_match_loading() {
    for input in [b"not a stream".as_slice(), &[0x78, 0x9c, 0x73]] {
        let mut cmd = command(input);
        transmission_mut(&mut cmd).compression = Compression::ZlibDeflate;
        assert_error(&cmd, ImageError::DecompressionFailed);
    }
    let input = [
        0x78, 0x9c, 0x73, 0x74, 0x72, 0x06, 0x00, 0x01, 0x8d, 0x00, 0xc7,
    ];
    let mut cmd = command(&input[..3]);
    transmission_mut(&mut cmd).compression = Compression::ZlibDeflate;
    let mut loading = LoadingImage::init(&cmd, Limits::DIRECT).unwrap();
    loading.add_data(&input[3..]).unwrap();
    loading.add_data(b"extra").unwrap();
    let image = loading.complete().unwrap();
    assert_eq!(image.data, b"ABC");
    assert_eq!(image.compression, Compression::None);
}

#[test]
fn zlib_integrity_failures_do_not_produce_images() {
    let input = [
        0x78, 0x9c, 0x73, 0x74, 0x72, 0x06, 0x00, 0x01, 0x8d, 0x00, 0xc7,
    ];
    let mut cmd = command(&input);
    transmission_mut(&mut cmd).compression = Compression::ZlibDeflate;
    assert_eq!(complete(&cmd).unwrap().data, b"ABC");
    for (index, byte) in [(10, 0xc6), (1, 0x9d)] {
        let mut corrupt = input;
        corrupt[index] = byte;
        cmd.data = corrupt.to_vec();
        assert_error(&cmd, ImageError::DecompressionFailed);
    }
}

#[test]
fn loading_retains_owned_first_chunk_state() {
    let cmd = Parser::parse_string(b"a=T,i=12,I=13,p=14,q=2,s=1,v=1,f=24,x=3,r=7;QQ==").unwrap();
    let mut loading = LoadingImage::init(&cmd, Limits::DIRECT).unwrap();
    let display = cmd.display().copied();
    drop(cmd);
    loading.add_data(b"BC").unwrap();
    let image = loading.complete().unwrap();
    assert_eq!(
        (
            image.id,
            image.number,
            image.width,
            image.height,
            image.format,
            image.compression,
            image.generation,
            image.implicit_id
        ),
        (12, 13, 1, 1, Format::Rgb, Compression::None, 0, false)
    );
    assert_eq!(image.data, b"ABC");
    assert_eq!(loading.image, Image::default());
    assert!(loading.data.is_empty());
    assert_eq!(loading.display, display);
    assert_eq!(loading.quiet, Quiet::Failures);
}

#[test]
fn temporary_file_requires_its_protocol_name() {
    let scratch = Scratch::new().unwrap();
    let path = fixture_file(&scratch, "ordinary.rgb", b"ABC");
    assert_error(
        &local(&path, Medium::TemporaryFile),
        ImageError::TemporaryFileNotNamedCorrectly,
    );
    assert_eq!(fs::read(path).unwrap(), b"ABC");
}

#[test]
fn temporary_file_pixels_are_loaded_and_removed() {
    let scratch = Scratch::new().unwrap();
    let path = fixture_file(&scratch, "tty-graphics-protocol.rgb", b"ABC");
    assert_eq!(
        complete(&local(&path, Medium::TemporaryFile)).unwrap().data,
        b"ABC"
    );
    assert!(!path.exists());
}

#[test]
fn regular_file_pixels_are_loaded_and_retained() {
    let scratch = Scratch::new().unwrap();
    let path = fixture_file(&scratch, "ordinary.rgb", b"ABC");
    assert_eq!(complete(&local(&path, Medium::File)).unwrap().data, b"ABC");
    assert_eq!(fs::read(path).unwrap(), b"ABC");
}

#[test]
fn direct_load_ignores_local_medium_flags() {
    assert_eq!(
        LoadingImage::init(&command(b"ABC"), Limits::DIRECT)
            .unwrap()
            .complete()
            .unwrap()
            .data,
        b"ABC"
    );
}

#[test]
fn file_load_honours_disabled_file_flag() {
    let scratch = Scratch::new().unwrap();
    let path = fixture_file(&scratch, "ordinary.rgb", b"ABC");
    let error = LoadingImage::init(
        &local(&path, Medium::File),
        Limits {
            file: false,
            temporary_file: true,
            shared_memory: true,
        },
    )
    .unwrap_err();
    assert_eq!(error, ImageError::UnsupportedMedium);
    assert_eq!(fs::read(path).unwrap(), b"ABC");
}

#[test]
fn file_load_honours_enabled_file_flag() {
    let scratch = Scratch::new().unwrap();
    let path = fixture_file(&scratch, "ordinary.rgb", b"ABC");
    assert_eq!(
        LoadingImage::init(
            &local(&path, Medium::File),
            Limits {
                file: true,
                temporary_file: false,
                shared_memory: false
            }
        )
        .unwrap()
        .complete()
        .unwrap()
        .data,
        b"ABC"
    );
}

#[test]
fn temporary_load_honours_disabled_temporary_flag() {
    let scratch = Scratch::new().unwrap();
    let path = fixture_file(&scratch, "tty-graphics-protocol.rgb", b"ABC");
    assert_eq!(
        LoadingImage::init(
            &local(&path, Medium::TemporaryFile),
            Limits {
                file: true,
                temporary_file: false,
                shared_memory: true
            }
        )
        .unwrap_err(),
        ImageError::UnsupportedMedium
    );
    assert_eq!(fs::read(path).unwrap(), b"ABC");
}

#[test]
fn temporary_load_honours_enabled_temporary_flag() {
    let scratch = Scratch::new().unwrap();
    let path = fixture_file(&scratch, "tty-graphics-protocol.rgb", b"ABC");
    assert_eq!(
        LoadingImage::init(
            &local(&path, Medium::TemporaryFile),
            Limits {
                file: false,
                temporary_file: true,
                shared_memory: false
            }
        )
        .unwrap()
        .complete()
        .unwrap()
        .data,
        b"ABC"
    );
    assert!(!path.exists());
}

#[test]
fn regular_file_ranges_and_path_failures_are_observable() {
    #[cfg(unix)]
    assert_native_file_bytes();

    let scratch = Scratch::new().unwrap();
    let path = fixture_file(&scratch, "pixels.rgb", b"ABC");
    for (offset, size, expected) in [
        (0, 0, b"ABC".as_slice()),
        (1, 0, b"BC".as_slice()),
        (1, 2, b"BC".as_slice()),
        (9, 0, b"".as_slice()),
        (0, 10, b"ABC".as_slice()),
    ] {
        let mut cmd = local(&path, Medium::File);
        let t = transmission_mut(&mut cmd);
        t.offset = offset;
        t.size = size;
        assert_eq!(
            LoadingImage::init(&cmd, Limits::ALL).unwrap().data,
            expected
        );
    }
    let mut cmd = local(&path, Medium::File);
    transmission_mut(&mut cmd).size = 2;
    assert_eq!(
        LoadingImage::init(&cmd, Limits::ALL).unwrap_err(),
        ImageError::InvalidData
    );
    for path in [scratch.path().join("missing"), scratch.path().to_path_buf()] {
        assert_error(&local(&path, Medium::File), ImageError::InvalidData);
    }
    let mut cmd = command(b"invalid\0path");
    transmission_mut(&mut cmd).medium = Medium::File;
    assert_error(&cmd, ImageError::InvalidData);
    #[cfg(target_os = "linux")]
    for path in ["/proc/version", "/sys/kernel/uevent_seqnum", "/dev/null"] {
        assert_error(
            &local(Path::new(path), Medium::File),
            ImageError::InvalidData,
        );
    }
    #[cfg(target_os = "linux")]
    {
        let shared_scratch = Scratch::new_in(Path::new("/dev/shm")).unwrap();
        let path = fixture_file(&shared_scratch, "pixels.rgb", b"ABC");
        let image = complete(&local(&path, Medium::File)).unwrap();
        let data = fs::read(&path).unwrap();
        assert_eq!(image.data, b"ABC");
        assert_eq!(data, b"ABC");
    }
}

#[test]
fn temporary_aliases_and_failure_cleanup_are_safe() {
    let scratch = Scratch::new().unwrap();
    let path = fixture_file(&scratch, "tty-graphics-protocol-overflow.rgb", b"ABCD");
    let mut cmd = local(&path, Medium::TemporaryFile);
    transmission_mut(&mut cmd).size = 3;
    assert_error(&cmd, ImageError::InvalidData);
    assert!(!path.exists());
    let directory = scratch.path().join("tty-graphics-protocol-dir");
    fs::create_dir(&directory).unwrap();
    assert_error(
        &local(&directory, Medium::TemporaryFile),
        ImageError::InvalidData,
    );
    assert!(directory.is_dir());
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let real = fixture_file(&scratch, "tty-graphics-protocol-real.rgb", b"ABC");
        let alias = scratch.path().join("alias");
        symlink(&real, &alias).unwrap();
        assert_eq!(
            complete(&local(&alias, Medium::TemporaryFile))
                .unwrap()
                .data,
            b"ABC"
        );
        assert!(!real.exists());
        assert!(alias.symlink_metadata().is_ok());
        let target = scratch.path().join("parent");
        fs::create_dir(&target).unwrap();
        let parent_alias = scratch.path().join("parent-alias");
        symlink(&target, &parent_alias).unwrap();
        let real = fixture_file(&scratch, "parent/tty-graphics-protocol-parent.rgb", b"ABC");
        assert_eq!(
            complete(&local(
                &parent_alias.join("tty-graphics-protocol-parent.rgb"),
                Medium::TemporaryFile
            ))
            .unwrap()
            .data,
            b"ABC"
        );
        assert!(!real.exists());
    }
}

#[test]
fn png_file_overrides_declared_geometry() {
    let scratch = Scratch::new().unwrap();
    let path = fixture_file(
        &scratch,
        "sample.png",
        include_bytes!("fixtures/images/sample50x76.png"),
    );
    let mut cmd = local(&path, Medium::File);
    let t = transmission_mut(&mut cmd);
    t.format = Format::Png;
    t.width = 2;
    t.height = 3;
    t.image_id = 17;
    let image = complete(&cmd).unwrap();
    assert_eq!(
        (image.width, image.height, image.format, image.id),
        (50, 76, Format::Rgba, 17)
    );
    assert_eq!(
        image.data,
        include_bytes!("fixtures/images/sample50x76.rgba")
    );
    assert!(path.exists());
}

fn png_pixels(bytes: &[u8]) -> Vec<u8> {
    let mut cmd = command(bytes);
    transmission_mut(&mut cmd).format = Format::Png;
    let image = complete(&cmd).unwrap();
    assert_eq!(
        (image.width, image.height, image.format),
        (1, 1, Format::Rgba)
    );
    image.data
}

#[test]
fn png_colour_channels_expand_to_rgba() {
    for (bytes, expected) in [
        (
            include_bytes!("fixtures/images/gray8.png").as_slice(),
            [127, 127, 127, 255],
        ),
        (
            include_bytes!("fixtures/images/gray_alpha8.png").as_slice(),
            [127, 127, 127, 63],
        ),
        (
            include_bytes!("fixtures/images/rgb8.png").as_slice(),
            [12, 34, 56, 255],
        ),
        (
            include_bytes!("fixtures/images/rgba8.png").as_slice(),
            [12, 34, 56, 78],
        ),
    ] {
        assert_eq!(png_pixels(bytes), expected);
    }
}

#[test]
fn png_palette_entries_expand_before_format_validation() {
    assert_eq!(
        png_pixels(include_bytes!("fixtures/images/palette8.png")),
        [11, 22, 33, 255]
    );
    assert_eq!(
        png_pixels(include_bytes!("fixtures/images/palette_alpha.png")),
        [11, 22, 33, 44]
    );
}

#[test]
fn png_bit_depths_expand_and_strip_exactly() {
    for (bytes, expected) in [
        (
            include_bytes!("fixtures/images/gray1.png").as_slice(),
            [255, 255, 255, 255],
        ),
        (
            include_bytes!("fixtures/images/gray16.png").as_slice(),
            [18, 18, 18, 255],
        ),
        (
            include_bytes!("fixtures/images/rgba16.png").as_slice(),
            [1, 3, 5, 7],
        ),
    ] {
        assert_eq!(png_pixels(bytes), expected);
    }
    let mut cmd = command(include_bytes!("fixtures/images/invalid.png"));
    transmission_mut(&mut cmd).format = Format::Png;
    let mut loading = LoadingImage::init(&cmd, Limits::DIRECT).unwrap();
    assert_eq!(loading.complete(), Err(ImageError::InvalidData));
}

#[test]
fn png_geometry_is_limited_before_decoding() {
    // The one-pixel fixture with a header claiming 1,000,000 x 1,000,000 pixels.
    let mut bytes = include_bytes!("fixtures/images/rgba8.png").to_vec();
    bytes[16..24].copy_from_slice(&[0, 0x0f, 0x42, 0x40, 0, 0x0f, 0x42, 0x40]);
    let mut crc = flate2::Crc::new();
    crc.update(&bytes[12..29]);
    bytes[29..33].copy_from_slice(&crc.sum().to_be_bytes());
    let mut cmd = command(&bytes);
    transmission_mut(&mut cmd).format = Format::Png;
    assert_error(&cmd, ImageError::InvalidData);
}

#[test]
#[cfg(target_os = "linux")]
fn shared_memory_pixels_trim_alignment_padding() {
    for slash in [true, false] {
        let segment = Segment::new(b"ABCpadding");
        let mut cmd = segment.command();
        if !slash {
            cmd.data.remove(0);
        }
        let image = complete(&cmd).unwrap();
        assert_eq!(image.data, b"ABC");
        assert!(!segment.exists());
    }
}

#[test]
#[cfg(target_os = "linux")]
fn shared_memory_images_stop_at_the_image_limit() {
    // A valid PNG, then sparse padding past the 400 MiB limit or far beyond any allocation.
    for size in [400 * 1024 * 1024 + 1, 1 << 40] {
        let segment = Segment::new(include_bytes!("fixtures/images/rgba8.png"));
        let flags = rustix::shm::OFlags::RDWR;
        File::from(
            rustix::shm::open(segment.0.as_str(), flags, rustix::shm::Mode::empty()).unwrap(),
        )
        .set_len(size)
        .unwrap();
        let mut cmd = segment.command();
        transmission_mut(&mut cmd).format = Format::Png;
        assert_error(&cmd, ImageError::InvalidData);
        assert!(!segment.exists());
    }
}

#[test]
#[cfg(target_os = "linux")]
fn shared_memory_invalid_ranges_release_the_segment() {
    let root = Segment::new(b"ABC");
    let mut invalid = root.command();
    transmission_mut(&mut invalid).offset = 4;
    assert_eq!(
        LoadingImage::init(&invalid, Limits::ALL).unwrap_err(),
        ImageError::InvalidData
    );
    assert!(!root.exists());
    for (data, width, height, offset, size) in [
        (b"".as_slice(), 1, 1, 0, 0),
        (b"AB".as_slice(), 1, 1, 0, 0),
        (b"ABC".as_slice(), 0, 1, 0, 0),
        (b"ABC".as_slice(), 1, 0, 0, 0),
        (b"ABC".as_slice(), 10001, 1, 0, 0),
        (b"ABC".as_slice(), 1, 10001, 0, 0),
        (b"ABC".as_slice(), u32::MAX, u32::MAX, 0, 0),
        (b"ABC".as_slice(), 1, 1, 3, 0),
        (b"ABC".as_slice(), 1, 1, u32::MAX, u32::MAX),
        (b"ABC".as_slice(), 1, 1, 0, 2),
        (b"ABC".as_slice(), 1, 1, 1, 0),
    ] {
        let segment = Segment::new(data);
        let mut cmd = segment.command();
        let t = transmission_mut(&mut cmd);
        t.width = width;
        t.height = height;
        t.offset = offset;
        t.size = size;
        assert_error(&cmd, ImageError::InvalidData);
        assert!(!segment.exists());
    }
    let control = Segment::new(b"ABC");
    assert_eq!(complete(&control.command()).unwrap().data, b"ABC");
    assert!(!control.exists());
    for name in [
        b"/missing-maestro-graphics-name".as_slice(),
        b"/invalid/name".as_slice(),
        b"/invalid\0name".as_slice(),
    ] {
        let mut cmd = command(name);
        transmission_mut(&mut cmd).medium = Medium::SharedMemory;
        assert_error(&cmd, ImageError::InvalidData);
    }
    let unopened = Segment::new(b"ABC");
    let mut invalid = unopened.command();
    invalid.data.extend_from_slice(b"/not-a-native-name");
    assert_error(&invalid, ImageError::InvalidData);
    assert!(unopened.exists());
}

#[test]
fn medium_flags_are_independent_and_platform_fallbacks_hold() {
    #[cfg(target_os = "linux")]
    {
        let segment = Segment::new(b"ABC");
        assert_eq!(
            LoadingImage::init(
                &segment.command(),
                Limits {
                    file: true,
                    temporary_file: true,
                    shared_memory: false
                }
            )
            .unwrap_err(),
            ImageError::UnsupportedMedium
        );
        assert!(segment.exists());
        assert_eq!(
            LoadingImage::init(
                &segment.command(),
                Limits {
                    file: false,
                    temporary_file: false,
                    shared_memory: true
                }
            )
            .unwrap()
            .complete()
            .unwrap()
            .data,
            b"ABC"
        );
        assert!(!segment.exists());
    }
    #[cfg(not(target_os = "linux"))]
    {
        let mut cmd = command(b"/maestro-image-platform-fallback");
        transmission_mut(&mut cmd).medium = Medium::SharedMemory;
        assert_error(&cmd, ImageError::UnsupportedMedium);
    }
}

#[test]
fn image_data_limits_are_enforced_at_the_owner() {
    const LIMIT: usize = 400 * 1024 * 1024;
    {
        let mut loading = LoadingImage::init(&command(b""), Limits::DIRECT).unwrap();
        loading.add_data(b"").unwrap();
        assert!(loading.data.is_empty());
        loading.add_data(&vec![0; LIMIT]).unwrap();
        assert_eq!(loading.data.len(), LIMIT);
        assert_eq!(loading.add_data(&[1]), Err(ImageError::InvalidData));
        assert_eq!(loading.data.len(), LIMIT);
    }
    {
        let scratch = Scratch::new().unwrap();
        let path = fixture_file(&scratch, "excess.rgb", b"");
        OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(u64::try_from(LIMIT).unwrap() + 1)
            .unwrap();
        assert_eq!(
            LoadingImage::init(&local(&path, Medium::File), Limits::ALL).unwrap_err(),
            ImageError::InvalidData
        );
    }
    {
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::copy(
            &mut std::io::Read::take(std::io::repeat(0), u64::try_from(LIMIT).unwrap() + 1),
            &mut encoder,
        )
        .unwrap();
        let mut cmd = command(&encoder.finish().unwrap());
        transmission_mut(&mut cmd).compression = Compression::ZlibDeflate;
        assert_error(&cmd, ImageError::DecompressionFailed);
    }
    {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 10241, 10241);
        encoder.set_compression(png::Compression::Fast);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        {
            let mut writer = encoder.write_header().unwrap();
            let mut stream = writer.stream_writer().unwrap();
            let row = vec![0; 10241 * 4];
            for _ in 0..10241 {
                stream.write_all(&row).unwrap();
            }
            stream.finish().unwrap();
        }
        let mut cmd = command(&bytes);
        transmission_mut(&mut cmd).format = Format::Png;
        assert_error(&cmd, ImageError::InvalidData);
    }
}

/// Observe non-Unicode native path bytes through the public loader.
#[cfg(unix)]
fn assert_native_file_bytes() {
    use std::os::unix::ffi::OsStrExt as _;
    let scratch = Scratch::new().unwrap();
    let path = fixture_file(
        &scratch,
        std::ffi::OsStr::from_bytes(b"pixels-\xff.rgb"),
        b"ABC",
    );
    assert_eq!(complete(&local(&path, Medium::File)).unwrap().data, b"ABC");
    assert_eq!(fs::read(path).unwrap(), b"ABC");
}
