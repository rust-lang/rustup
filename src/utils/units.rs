use std::fmt::{self, Display};

#[derive(Copy, Clone, Debug)]
pub enum Unit {
    B,
}

pub(crate) enum UnitMode {
    Norm,
}

/// Human readable size (some units)
pub(crate) enum Size {
    B(usize, UnitMode),
}

impl Size {
    pub(crate) fn new(size: usize, unit: Unit, unitmode: UnitMode) -> Self {
        match unit {
            Unit::B => Self::B(size, unitmode),
        }
    }
}

impl Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Size::B(size, unitmode) => {
                const KI: f64 = 1024.0;
                const MI: f64 = KI * KI;
                const GI: f64 = KI * KI * KI;
                let size = *size as f64;

                let suffix: String = match unitmode {
                    UnitMode::Norm => "".into(),
                };

                if size >= GI {
                    write!(f, "{:5.1} GiB{}", size / GI, suffix)
                } else if size >= MI {
                    write!(f, "{:5.1} MiB{}", size / MI, suffix)
                } else if size >= KI {
                    write!(f, "{:5.1} KiB{}", size / KI, suffix)
                } else {
                    write!(f, "{size:3.0} B{suffix}")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rustup_macros::unit_test as test;

    #[test]
    fn unit_formatter_test() {
        use crate::utils::units::{Size, Unit, UnitMode};

        // Test Bytes
        assert_eq!(
            format!("{}", Size::new(1, Unit::B, UnitMode::Norm)),
            "  1 B"
        );
        assert_eq!(
            format!("{}", Size::new(1024, Unit::B, UnitMode::Norm)),
            "  1.0 KiB"
        );
        assert_eq!(
            format!("{}", Size::new(1024usize.pow(2), Unit::B, UnitMode::Norm)),
            "  1.0 MiB"
        );
        assert_eq!(
            format!("{}", Size::new(1024usize.pow(3), Unit::B, UnitMode::Norm)),
            "  1.0 GiB"
        );
    }
}
