use yew::prelude::*;

// QR Type
#[derive(Clone, PartialEq, Eq)]
pub enum QrType {
    Canvas,
    Svg,
}

impl From<QrType> for &'static str {
    fn from(value: QrType) -> Self {
        match value {
            QrType::Canvas => "canvas",
            QrType::Svg => "svg",
        }
    }
}

// QR Dots Type
#[derive(Clone, PartialEq, Eq)]
pub enum QrDotsType {
    Square,
    Dots,
    Rounded,
    Classy,
    ClassyRounded,
    ExtraRounded,
}

impl From<QrDotsType> for &'static str {
    fn from(value: QrDotsType) -> Self {
        match value {
            QrDotsType::Square => "square",
            QrDotsType::Dots => "dots",
            QrDotsType::Rounded => "rounded",
            QrDotsType::Classy => "classy",
            QrDotsType::ClassyRounded => "classy-rounded",
            QrDotsType::ExtraRounded => "extra-rounded",
        }
    }
}

// QR Corners Square Type
#[derive(Clone, PartialEq, Eq)]
pub enum QrCornersSquareType {
    Square,
    ExtraRounded,
    Dot,
}

impl From<QrCornersSquareType> for &'static str {
    fn from(value: QrCornersSquareType) -> Self {
        match value {
            QrCornersSquareType::Square => "square",
            QrCornersSquareType::ExtraRounded => "extra-rounded",
            QrCornersSquareType::Dot => "dot",
        }
    }
}

// QR Corners Dot Type
#[derive(Clone, PartialEq, Eq)]
pub enum QrCornersDotType {
    Square,
    Dot,
}

impl From<QrCornersDotType> for &'static str {
    fn from(value: QrCornersDotType) -> Self {
        match value {
            QrCornersDotType::Square => "square",
            QrCornersDotType::Dot => "dot",
        }
    }
}

// QR Error Correction Level
#[derive(Clone, PartialEq, Eq)]
pub enum QrErrorCorrectionLevel {
    L,
    M,
    Q,
    H,
}

impl From<QrErrorCorrectionLevel> for &'static str {
    fn from(value: QrErrorCorrectionLevel) -> Self {
        match value {
            QrErrorCorrectionLevel::L => "L",
            QrErrorCorrectionLevel::M => "M",
            QrErrorCorrectionLevel::Q => "Q",
            QrErrorCorrectionLevel::H => "H",
        }
    }
}

// QR Mode
#[derive(Clone, PartialEq, Eq)]
pub enum QrMode {
    Numeric,
    Alphanumeric,
    Byte,
    Kanji,
}

impl From<QrMode> for &'static str {
    fn from(value: QrMode) -> Self {
        match value {
            QrMode::Numeric => "Numeric",
            QrMode::Alphanumeric => "Alphanumeric",
            QrMode::Byte => "Byte",
            QrMode::Kanji => "Kanji",
        }
    }
}

// QR Shape
#[derive(Clone, PartialEq, Eq)]
pub enum QrShape {
    Square,
    Circle,
}

impl From<QrShape> for &'static str {
    fn from(value: QrShape) -> Self {
        match value {
            QrShape::Square => "square",
            QrShape::Circle => "circle",
        }
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct BitcoinQrCodeProps {
    pub id: String,

    // Payment methods (at least one must be provided)
    #[prop_or_default]
    pub unified: Option<String>,
    #[prop_or_default]
    pub bitcoin: Option<String>,
    #[prop_or_default]
    pub lightning: Option<String>,
    #[prop_or_default]
    pub parameters: Option<String>,

    // Dimensions and type
    #[prop_or_default]
    pub width: Option<String>,
    #[prop_or_default]
    pub height: Option<String>,
    #[prop_or_default]
    pub type_: Option<QrType>,
    #[prop_or_default]
    pub margin: Option<u32>,

    // Polling and debug options
    #[prop_or_default]
    pub is_polling: Option<bool>,
    #[prop_or_default]
    pub poll_interval: Option<u32>,
    #[prop_or_default]
    pub debug: Option<bool>,

    // Image options
    #[prop_or_default]
    pub image: Option<String>,
    #[prop_or_default]
    pub image_embedded: Option<bool>,
    #[prop_or_default]
    pub image_hide_background_dots: Option<bool>,
    #[prop_or_default]
    pub image_size: Option<f64>,
    #[prop_or_default]
    pub image_cross_origin: Option<String>,
    #[prop_or_default]
    pub image_margin: Option<u32>,

    // QR code technical options
    #[prop_or_default]
    pub shape: Option<QrShape>,
    #[prop_or_default]
    pub qr_type_number: Option<u32>,
    #[prop_or_default]
    pub qr_mode: Option<QrMode>,
    #[prop_or_default]
    pub qr_error_correction_level: Option<QrErrorCorrectionLevel>,

    // Style options
    #[prop_or_default]
    pub dots_type: Option<QrDotsType>,
    #[prop_or_default]
    pub dots_color: Option<String>,
    #[prop_or_default]
    pub dots_rotation: Option<f64>,
    #[prop_or_default]
    pub corners_square_type: Option<QrCornersSquareType>,
    #[prop_or_default]
    pub corners_square_color: Option<String>,
    #[prop_or_default]
    pub corners_dot_type: Option<QrCornersDotType>,
    #[prop_or_default]
    pub corners_dot_color: Option<String>,
    #[prop_or_default]
    pub background_round: Option<u32>,
    #[prop_or_default]
    pub background_color: Option<String>,
}

#[function_component(BitcoinQrCode)]
pub fn bitcoin_qr(props: &BitcoinQrCodeProps) -> Html {
    html! {
        <bitcoin-qr
            id={props.id.clone()}

            // Payment methods
            unified={props.unified.clone()}
            bitcoin={props.bitcoin.clone()}
            lightning={props.lightning.clone()}
            parameters={props.parameters.clone()}

            // Dimensions and type
            width={props.width.clone()}
            height={props.height.clone()}
            type={props.type_.as_ref().map(|t| Into::<&str>::into(t.clone()))}
            margin={props.margin.map(|m| m.to_string())}

            // Polling and debug options
            is-polling={props.is_polling.map(|p| p.to_string())}
            poll-interval={props.poll_interval.map(|p| p.to_string())}
            debug={props.debug.map(|d| d.to_string())}

            // Image options
            image={props.image.clone()}
            image-embedded={props.image_embedded.map(|e| e.to_string())}
            image-hide-background-dots={props.image_hide_background_dots.map(|h| h.to_string())}
            image-size={props.image_size.map(|s| s.to_string())}
            image-cross-origin={props.image_cross_origin.clone()}
            image-margin={props.image_margin.map(|m| m.to_string())}

            // QR code technical options
            shape={props.shape.as_ref().map(|s| Into::<&str>::into(s.clone()))}
            qr-type-number={props.qr_type_number.map(|n| n.to_string())}
            qr-mode={props.qr_mode.as_ref().map(|m| Into::<&str>::into(m.clone()))}
            qr-error-correction-level={props.qr_error_correction_level.as_ref().map(|l| Into::<&str>::into(l.clone()))}

            // Style options
            dots-type={props.dots_type.as_ref().map(|t| Into::<&str>::into(t.clone()))}
            dots-color={props.dots_color.clone()}
            dots-rotation={props.dots_rotation.map(|r| r.to_string())}
            corners-square-type={props.corners_square_type.as_ref().map(|t| Into::<&str>::into(t.clone()))}
            corners-square-color={props.corners_square_color.clone()}
            corners-dot-type={props.corners_dot_type.as_ref().map(|t| Into::<&str>::into(t.clone()))}
            corners-dot-color={props.corners_dot_color.clone()}
            background-round={props.background_round.map(|r| r.to_string())}
            background-color={props.background_color.clone()}
        />
    }
}
