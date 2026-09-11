use super::block::BasicBlock;
use super::inst::{BlockId, InstData, InstId, Instruction, Operand};
use super::types::Type;
use types::TypeDatabase;

pub struct IrBuilder<'a> {
    pub blocks: &'a mut Vec<BasicBlock>,
    pub instructions: &'a mut Vec<InstData>,
    pub current_block: Option<BlockId>,
    pub type_db: &'a mut TypeDatabase,
}

impl<'a> IrBuilder<'a> {
    pub fn new(
        blocks: &'a mut Vec<BasicBlock>,
        instructions: &'a mut Vec<InstData>,
        type_db: &'a mut TypeDatabase,
    ) -> Self {
        let current_block = if blocks.is_empty() {
            None
        } else {
            Some(blocks.last().unwrap().id)
        };
        Self {
            blocks,
            instructions,
            current_block,
            type_db,
        }
    }

    pub fn set_block(&mut self, block: BlockId) {
        self.current_block = Some(block);
    }

    pub fn create_block(&mut self) -> BlockId {
        let id = BlockId(self.blocks.len() as u32);
        self.blocks.push(BasicBlock::new(id));
        id
    }

    pub fn build_inst(&mut self, inst: Instruction, ty: Type) -> InstId {
        let inst_id = InstId(self.instructions.len() as u32);
        self.instructions.push(InstData { inst, ty });
        if let Some(block_id) = self.current_block {
            self.blocks[block_id.0 as usize].instructions.push(inst_id);
        }
        inst_id
    }

    pub fn build_alloca(&mut self, ty: Type) -> Operand {
        let ptr_ty = self.type_db.pointer(ty);
        Operand::Reg(self.build_inst(Instruction::Alloca(ty), ptr_ty))
    }

    pub fn build_load(&mut self, ty: Type, ptr: Operand) -> Operand {
        Operand::Reg(self.build_inst(Instruction::Load(ptr), ty))
    }

    pub fn build_store(&mut self, ty: Type, ptr: Operand, value: Operand) -> InstId {
        let void = self.type_db.void();
        self.build_inst(Instruction::Store(ty, ptr, value), void)
    }

    pub fn build_br(&mut self, target: BlockId) -> InstId {
        let void = self.type_db.void();
        self.build_inst(Instruction::Br(target), void)
    }

    pub fn build_cond_br(
        &mut self,
        cond: Operand,
        true_block: BlockId,
        false_block: BlockId,
    ) -> InstId {
        let void = self.type_db.void();
        self.build_inst(Instruction::CondBr(cond, true_block, false_block), void)
    }

    pub fn build_return(&mut self, val: Option<Operand>) -> InstId {
        let void = self.type_db.void();
        self.build_inst(Instruction::Return(val), void)
    }
}
