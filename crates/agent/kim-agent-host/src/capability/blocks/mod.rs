mod bash;
mod fs;
mod im;
mod mcp;
mod subagent;

use std::collections::HashMap;
use std::sync::Arc;

use crate::capability::CapabilityBlock;

pub fn build_registry() -> HashMap<&'static str, Arc<dyn CapabilityBlock>> {
    let mut map: HashMap<&'static str, Arc<dyn CapabilityBlock>> = HashMap::new();
    for block in im::all()
        .into_iter()
        .chain(std::iter::once(
            Arc::new(fs::FsBlock) as Arc<dyn CapabilityBlock>
        ))
        .chain(std::iter::once(
            Arc::new(bash::BashBlock) as Arc<dyn CapabilityBlock>
        ))
        .chain(std::iter::once(
            Arc::new(mcp::McpBlock) as Arc<dyn CapabilityBlock>
        ))
        .chain(std::iter::once(
            Arc::new(subagent::SubagentBlock) as Arc<dyn CapabilityBlock>
        ))
    {
        map.insert(block.kind(), block);
    }
    map
}
