use lex_just_parse::lexer::TokenSource;

use super::block::BasicBlock;
use super::inst::{BlockId, InstData};
use super::types::Type;

#[derive(Debug, Clone)]
pub struct Signature {
    pub name: TokenSource,
    pub has_va_args: bool,
    pub params: Vec<Type>,
    pub return_type: Type,
    pub is_private: bool,
}

#[derive(Debug, Clone)]
pub enum Function {
    Foreign {
        name: TokenSource,
        link_name: TokenSource,
        signature: Signature,
    },
    Default {
        name: TokenSource,
        signature: Signature,
        blocks: Vec<BasicBlock>,
        instructions: Vec<InstData>,
        entry_block: BlockId,
        is_export: bool,
        link_name: Option<TokenSource>,
    },
}

impl Function {
    pub fn new(
        name: TokenSource,
        signature: Signature,
        is_export: bool,
        link_name: Option<TokenSource>,
    ) -> Self {
        let entry_block = BasicBlock::new(BlockId(0));
        Self::Default {
            name,
            signature,
            blocks: vec![entry_block],
            instructions: Vec::new(),
            entry_block: BlockId(0),
            is_export,
            link_name,
        }
    }

    pub fn foreign(name: TokenSource, link_name: TokenSource, signature: Signature) -> Self {
        Self::Foreign {
            name,
            link_name,
            signature,
        }
    }

    pub fn name(&self) -> &TokenSource {
        match self {
            Function::Foreign { name, .. } => name,
            Function::Default { name, .. } => name,
        }
    }

    pub fn symbol_name(&self) -> String {
        match self {
            Function::Foreign { link_name, .. } => link_name.to_string(),
            Function::Default {
                is_export: true,
                link_name: Some(link_name),
                ..
            } => link_name.to_string(),
            Function::Default {
                signature,
                is_export: true,
                link_name: None,
                ..
            } => {
                if signature.name == "main" {
                    "chs_main".to_string()
                } else {
                    signature.name.to_string()
                }
            }
            Function::Default { name, .. } => {
                if name == "main" || name == "main.main" {
                    "chs_main".to_string()
                } else {
                    format!("chs_{}", name.replace('.', "_"))
                }
            }
        }
    }

    pub fn signature(&self) -> &Signature {
        match self {
            Function::Foreign { signature, .. } => signature,
            Function::Default { signature, .. } => signature,
        }
    }

    pub fn is_default(&self) -> bool {
        matches!(self, Self::Default { .. })
    }

    pub fn is_export(&self) -> bool {
        matches!(
            self,
            Self::Default {
                is_export: true,
                ..
            }
        )
    }

    /// Returns `true` if the function is [`Foreign`].
    ///
    /// [`Foreign`]: Function::Foreign
    #[must_use]
    pub fn is_foreign(&self) -> bool {
        matches!(self, Self::Foreign { .. })
    }
}
