use anyhow::anyhow;

#[derive(Debug, Default)]
pub struct Path<'a> {
    pub prefix_operators: Vec<PrefixOperator>,
    pub name: &'a str,
    pub postfix_operators: Vec<PostfixOperator<'a>>,
}

#[derive(Debug)]
pub enum PrefixOperator {
    Ref,
    Deref,
}

impl TryFrom<&str> for PrefixOperator {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "&" => Ok(PrefixOperator::Ref),
            "*" => Ok(PrefixOperator::Deref),
            _ => Err(anyhow!("invalid operator")),
        }
    }
}

impl From<&PrefixOperator> for char {
    fn from(value: &PrefixOperator) -> Self {
        match value {
            PrefixOperator::Ref => '&',
            PrefixOperator::Deref => '*',
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PostfixOperator<'a> {
    Field(&'a str),
    Index(usize),
}
