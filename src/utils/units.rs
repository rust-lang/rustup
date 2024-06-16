use std::fmt::{self, Display};

#[derive(Copy, Clone, Debug)]
pub enum Unit {
    B,
    IO,
}

pub(crate) enum UnitMode {
    Norm,
    Rate,
}

/// Human readable size (some units)
pub(crate) enum Size {
    B(usize, UnitMode),
    IO(usize, UnitMode),
}

impl Size {
    pub(crate) fn new(size: usize, unit: Unit, unitmode: UnitMode) -> Self {
        match unit {
            Unit::B => Self::B(size, unitmode),
            Unit::IO => Self::IO(size, unitmode),
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
                    UnitMode::Rate => "/s".into(),
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
            Size::IO(size, unitmode) => {
                const K: f64 = 1000.0;
                const M: f64 = K * K;
                const G: f64 = K * K * K;
                let size = *size as f64;

                let suffix: String = match unitmode {
                    UnitMode::Norm => "IO-ops".into(),
                    UnitMode::Rate => "IOPS".into(),
                };

                if size >= G {
                    write!(f, "{:5.1} giga-{}", size / G, suffix)
                } else if size >= M {
                    write!(f, "{:5.1} mega-{}", size / M, suffix)
                } else if size >= K {
                    write!(f, "{:5.1} kilo-{}", size / K, suffix)
                } else {
                    write!(f, "{size:3.0} {suffix}")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
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

        // Test Bytes at given rate
        assert_eq!(
            format!("{}", Size::new(1, Unit::B, UnitMode::Rate)),
            "  1 B/s"
        );
        assert_eq!(
            format!("{}", Size::new(1024, Unit::B, UnitMode::Rate)),
            "  1.0 KiB/s"
        );
        assert_eq!(
            format!("{}", Size::new(1024usize.pow(2), Unit::B, UnitMode::Rate)),
            "  1.0 MiB/s"
        );
        assert_eq!(
            format!("{}", Size::new(1024usize.pow(3), Unit::B, UnitMode::Rate)),
            "  1.0 GiB/s"
        );

        //Test I/O Operations
        assert_eq!(
            format!("{}", Size::new(1, Unit::IO, UnitMode::Norm)),
            "  1 IO-ops"
        );
        assert_eq!(
            format!("{}", Size::new(1000, Unit::IO, UnitMode::Norm)),
            "  1.0 kilo-IO-ops"
        );
        assert_eq!(
            format!("{}", Size::new(1000usize.pow(2), Unit::IO, UnitMode::Norm)),
            "  1.0 mega-IO-ops"
        );
        assert_eq!(
            format!("{}", Size::new(1000usize.pow(3), Unit::IO, UnitMode::Norm)),
            "  1.0 giga-IO-ops"
        );

        //Test I/O Operations at given rate
        assert_eq!(
            format!("{}", Size::new(1, Unit::IO, UnitMode::Rate)),
            "  1 IOPS"
        );
        assert_eq!(
            format!("{}", Size::new(1000, Unit::IO, UnitMode::Rate)),
            "  1.0 kilo-IOPS"
        );
        assert_eq!(
            format!("{}", Size::new(1000usize.pow(2), Unit::IO, UnitMode::Rate)),
            "  1.0 mega-IOPS"
        );
        assert_eq!(
            format!("{}", Size::new(1000usize.pow(3), Unit::IO, UnitMode::Rate)),
            "  1.0 giga-IOPS"
        );
    }
}
