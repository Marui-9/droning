use crate::message::base_message::{Message, MessageContent};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Error;
use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::str::from_utf8;
use wg_2024::network::NodeId;
use wg_2024::packet::{Fragment, FRAGMENT_DSIZE};

pub struct Assembler<M: MessageContent> {
    fragments: HashMap<u64, BTreeMap<u64, Fragment>>,
    phantom_data: PhantomData<M>,
}

impl<M: MessageContent + DeserializeOwned> Assembler<M> {
    pub fn new() -> Self {
        Assembler {
            fragments: HashMap::new(),
            phantom_data: PhantomData,
        }
    }
    pub fn compose_message(fragments: Vec<Fragment>) -> Result<Message<M>, Error> {
        let mut serialized = String::with_capacity(fragments.len() * FRAGMENT_DSIZE);
        for frag in fragments.iter() {
            serialized.push_str(from_utf8(&frag.data[..frag.length as usize]).unwrap());
        }
        Message::<M>::deserialize(serialized)
    }

    pub fn insert_fragment(
        &mut self,
        session_id: u64,
        fragment: Fragment,
    ) -> Option<Result<Message<M>, Error>> {
        let frag_count = fragment.total_n_fragments as usize;
        let frag_vec = self.fragments.entry(session_id).or_default();
        frag_vec.insert(fragment.fragment_index, fragment);
        if frag_vec.len() == frag_count {
            Some(Self::compose_message(frag_vec.values().cloned().collect()))
        } else {
            None
        }
    }

    pub fn forget(&mut self, session_id: u64) {
        self.fragments.remove(&session_id);
    }
}

#[derive(Debug)]
pub struct Disassembler<M: MessageContent> {
    fragments: HashMap<u64, BTreeMap<u64, Fragment>>,
    destinations: HashMap<u64, NodeId>,
    phantom_data: PhantomData<M>,
    last_session_id: u64,
}

impl<M: MessageContent + Serialize> Disassembler<M> {
    pub fn new() -> Self {
        Self {
            fragments: HashMap::new(),
            destinations: HashMap::new(),
            phantom_data: PhantomData,
            last_session_id: 0,
        }
    }

    fn decompose_message(message: Message<M>) -> BTreeMap<u64, Fragment> {
        let serialized = message.serialize();
        let bytes = serialized.into_bytes();
        let total_n_fragments = {
            let count = bytes.len() / FRAGMENT_DSIZE;
            if bytes.len() % FRAGMENT_DSIZE > 0 {
                count + 1
            } else {
                count
            }
        };
        let mut fragments = BTreeMap::new();
        for fragment_index in 0..total_n_fragments {
            let start = fragment_index * FRAGMENT_DSIZE;
            let end = ((fragment_index + 1) * FRAGMENT_DSIZE).min(bytes.len());
            let data = {
                let mut data = [0; FRAGMENT_DSIZE];
                data[..(end - start)].copy_from_slice(&bytes[start..end]);
                data
            };
            fragments.insert(
                fragment_index as u64,
                Fragment {
                    fragment_index: fragment_index as u64,
                    total_n_fragments: total_n_fragments as u64,
                    length: (end - start) as u8,
                    data,
                },
            );
        }
        fragments
    }

    pub fn get_fragment(&self, session_id: u64, fragment_index: u64) -> Option<Fragment> {
        self.fragments
            .get(&session_id)?
            .get(&fragment_index)
            .cloned()
    }

    pub fn disassembly(&mut self, message: Message<M>) -> Vec<Fragment> {
        let session_id = message.session_id;
        self.destinations.insert(session_id, message.destination_id);
        let fragments = Self::decompose_message(message);
        self.fragments.insert(session_id, fragments.clone());
        fragments.into_values().collect()
    }

    pub fn forget_fragment(&mut self, session_id: u64, fragment_index: u64) -> Option<Fragment> {
        if let Some(fragments) = self.fragments.get_mut(&session_id) {
            let removed = fragments.remove(&fragment_index);
            if fragments.is_empty() {
                self.fragments.remove(&session_id);
                self.destinations.remove(&session_id);
            }
            removed
        } else {
            None
        }
    }

    #[cfg(test)]
    pub fn has_fragments(&self, session_id: u64) -> bool {
        self.fragments.contains_key(&session_id)
    }

    pub fn new_session_id(&mut self) -> u64 {
        let new_session_id = self.last_session_id;
        self.last_session_id += 1;
        new_session_id
    }

    pub fn get_destination(&self, session_id: u64) -> Option<NodeId> {
        self.destinations.get(&session_id).cloned()
    }

    pub fn transform_session_id(session_id: u64, node_id: NodeId) -> u64 {
        (node_id as u64) << 56 | session_id
    }
}
