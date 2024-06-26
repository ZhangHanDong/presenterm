use crate::{
    media::{image::Image, printer::RegisterImageError},
    style::Color,
    theme::TypstStyle,
    tools::{ExecutionError, ThirdPartyTools},
    ImageRegistry,
};
use std::{fs, io, path::Path};
use tempfile::tempdir_in;

const DEFAULT_PPI: u32 = 300;
const DEFAULT_HORIZONTAL_MARGIN: u16 = 5;
const DEFAULT_VERTICAL_MARGIN: u16 = 7;

pub struct TypstRender {
    ppi: String,
    image_registry: ImageRegistry,
    root_dir: String,
}

impl TypstRender {
    pub fn new(ppi: u32, image_registry: ImageRegistry, root_dir: &Path) -> Self {
        // typst complains about empty paths so we give it a "." if we don't have one.
        let root_dir = match root_dir.to_string_lossy().to_string() {
            path if path.is_empty() => ".".into(),
            path => path,
        };
        Self { ppi: ppi.to_string(), image_registry, root_dir }
    }

    pub(crate) fn render_typst(&self, input: &str, style: &TypstStyle) -> Result<Image, TypstRenderError> {
        let workdir = tempdir_in(&self.root_dir)?;
        let mut typst_input = Self::generate_page_header(style)?;
        typst_input.push_str(input);

        let input_path = workdir.path().join("input.typst");
        fs::write(&input_path, &typst_input)?;
        self.render_to_image(workdir.path(), &input_path)
    }

    pub(crate) fn render_latex(&self, input: &str, style: &TypstStyle) -> Result<Image, TypstRenderError> {
        let output = ThirdPartyTools::pandoc(&["--from", "latex", "--to", "typst"])
            .stdin(input.as_bytes().into())
            .run_and_capture_stdout()?;

        let input = String::from_utf8_lossy(&output);
        self.render_typst(&input, style)
    }

    fn render_to_image(&self, base_path: &Path, path: &Path) -> Result<Image, TypstRenderError> {
        let output_path = base_path.join("output.png");
        ThirdPartyTools::typst(&[
            "compile",
            "--format",
            "png",
            "--root",
            &self.root_dir,
            "--ppi",
            &self.ppi,
            &path.to_string_lossy(),
            &output_path.to_string_lossy(),
        ])
        .run()?;

        let png_contents = fs::read(&output_path)?;
        let image = image::load_from_memory(&png_contents)?;
        let image = self.image_registry.register_image(image)?;
        Ok(image)
    }

    fn generate_page_header(style: &TypstStyle) -> Result<String, TypstRenderError> {
        let x_margin = style.horizontal_margin.unwrap_or(DEFAULT_HORIZONTAL_MARGIN);
        let y_margin = style.vertical_margin.unwrap_or(DEFAULT_VERTICAL_MARGIN);
        let background =
            style.colors.background.as_ref().map(Self::as_typst_color).unwrap_or_else(|| Ok(String::from("none")))?;
        let mut header = format!(
            "#set page(width: auto, height: auto, margin: (x: {x_margin}pt, y: {y_margin}pt), fill: {background})\n"
        );
        if let Some(color) = &style.colors.foreground {
            let color = Self::as_typst_color(color)?;
            header.push_str(&format!("#set text(fill: {color})\n"));
        }
        Ok(header)
    }

    fn as_typst_color(color: &Color) -> Result<String, TypstRenderError> {
        match color.as_rgb() {
            Some((r, g, b)) => Ok(format!("rgb(\"#{r:02x}{g:02x}{b:02x}\")")),
            None => Err(TypstRenderError::UnsupportedColor(color.to_string())),
        }
    }
}

impl Default for TypstRender {
    fn default() -> Self {
        Self::new(DEFAULT_PPI, Default::default(), Path::new("."))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TypstRenderError {
    #[error(transparent)]
    Execution(#[from] ExecutionError),

    #[error("io: {0}")]
    Io(#[from] io::Error),

    #[error("invalid image: {0}")]
    InvalidImage(#[from] image::ImageError),

    #[error("invalid image: {0}")]
    RegisterImage(#[from] RegisterImageError),

    #[error("unsupported color '{0}', only RGB is supported")]
    UnsupportedColor(String),
}
