use anyhow::Error;

pub trait Cursor {
    type Item;
    type Meta;

    fn peek(&self) -> Option<(&Self::Item, Self::Meta)>;

    #[allow(clippy::type_complexity)]
    fn advance(&mut self) -> Result<Option<(Self::Item, Self::Meta)>, Error>;
}
