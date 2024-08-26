pub trait Init {
    type Error: std::fmt::Display;
    type Config: Config;

    fn init(cfg: &Self::Config) -> Result<Self, Self::Error>
    where
        Self: Sized;
}

pub trait Config {}
