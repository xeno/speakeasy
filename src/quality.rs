#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputQuality {
    P720,
    P1080,
    P1440,
    P2160,
}

impl OutputQuality {
    pub const ALL: [Self; 4] = [Self::P720, Self::P1080, Self::P1440, Self::P2160];

    pub fn label(self) -> &'static str {
        match self {
            Self::P720 => "720p",
            Self::P1080 => "1080p",
            Self::P1440 => "1440p",
            Self::P2160 => "4K",
        }
    }

    /// Square side for the 16:9 crops (short side of the landscape frame).
    pub fn square_side(self) -> u32 {
        match self {
            Self::P720 => 720,
            Self::P1080 => 1080,
            Self::P1440 => 1440,
            Self::P2160 => 2160,
        }
    }

    pub fn capture_size(self) -> (u32, u32) {
        match self {
            Self::P720 => (1280, 720),
            Self::P1080 => (1920, 1080),
            Self::P1440 => (2560, 1440),
            Self::P2160 => (3840, 2160),
        }
    }

    pub fn from_label(label: &str) -> Self {
        match label {
            "720p" => Self::P720,
            "1440p" => Self::P1440,
            "4K" | "2160p" => Self::P2160,
            _ => Self::P1080,
        }
    }
}

impl Default for OutputQuality {
    fn default() -> Self {
        Self::P1080
    }
}

pub const PREVIEW_SIDE: u32 = 720;
