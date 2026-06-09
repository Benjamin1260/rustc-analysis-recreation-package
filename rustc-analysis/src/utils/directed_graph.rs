use std::{
    collections::{
        HashMap,
        HashSet,
    }, hash::Hash,
};

/* TODO: COULD make a single hashset instead of two seperate ones since the Keys should be equal between the two
could be implemented using HashSet<Key, (Incomming, Outgoing)>
where:
    Key: T
    Incommming: HashSet<T>
    Outgoing: HashSet<T> 
perhaps bundle these last 2 into a struct for readability and encapsulation
*/

#[derive(Debug)]
pub struct DirectedGraph<T>
where 
    T: Eq + Hash + Copy + std::fmt::Debug
{
    pub incomming: HashMap<T, HashSet<T>>, // implies edge: T <- Set<T>
    pub outgoing: HashMap<T, HashSet<T>>, // implies edge: T -> Set
}

impl<T> DirectedGraph<T> 
where 
    T: Eq + Hash + Copy + std::fmt::Debug
{
    /* mind all the special cases in following example:
    add_outgoing_from_iter(f1, [f2, f3]) -> {
        incomming {
            (f1, []), #1
            (f2, [f1]), #3
            (f3, [f1]), #3
        }
        outgoing {
            (f1, [f2, f3]), #2
            (f2, []), #4
            (f3, []), #4
        }
    }
    */

    // pub fn add_incomming_from_iter<I>(&mut self, from_nodes: I, to_node: T) 
    // where
    //     I: IntoIterator<Item=T> + Clone
    // {
    //     panic!("Not implemented!");
    // }

    pub fn add_outgoing_from_iter<I>(&mut self, from_node: T, to_nodes: I) 
    where
        I: IntoIterator<Item=T> + Clone
    {
        // TODO: COULD refactor this because the code reuse/duplication is insane
        assert!(self.check_invarients());

        // add `from_node` #1 and #2
        Self::create_empty_entry_if_not_present(&mut self.incomming, from_node);
        match self.outgoing.get_mut(&from_node) {
            Some(set) => set.extend(to_nodes.clone()),
            None => {self.outgoing.insert(from_node.clone(), HashSet::from_iter(to_nodes.clone()));},
        }

        // add `to_node` #3 and #4
        for to_node in to_nodes {
            self.add_incomming_only(from_node, to_node);
            Self::create_empty_entry_if_not_present(&mut self.outgoing, to_node);
        }

        assert!(self.check_invarients());
    }

    fn add_incomming_only(&mut self, from_node: T, to_node: T) {
        match self.incomming.get_mut(&to_node) {
            Some(set) => { set.insert(from_node); },
            None => {
                let mut hash_set:HashSet<T> = HashSet::new();
                hash_set.insert(from_node);
                self.incomming.insert(to_node, hash_set);
            },
        }
    }

    // fn add_outgoing_only(&mut self, from_node: T, to_node: T) {
    //     match self.outgoing.get_mut(&from_node) {
    //         Some(set) => { set.insert(to_node); },
    //         None => {
    //             let mut hash_set:HashSet<T> = HashSet::new();
    //             hash_set.insert(to_node);
    //             self.incomming.insert(from_node, hash_set);
    //         },
    //     }
    // }

    fn create_empty_entry_if_not_present(hash_set: &mut HashMap<T, HashSet<T>>, key: T) {
        if !hash_set.contains_key(&key) { hash_set.insert(key, HashSet::new()); }
    }

    // remove node from tree
    // get all incomming+outgoing edges
    // removing those from other nodes
    fn remove(&mut self, node: &T) {
        if let Some(incomming_nodes) = self.incomming.remove(&node) {
            for incomming_node in incomming_nodes {
                assert!(self.outgoing.get_mut(&incomming_node).unwrap().remove(&node)); // entry must exist and must succesfull remove
            }
        }

        if let Some(outgoing_nodes) = self.outgoing.remove(&node) {
            for outgoing_node in outgoing_nodes {
                assert!(self.incomming.get_mut(&outgoing_node).unwrap().remove(node)); // entry must exist and must succesfull remove
            }
        }
    }

    // checks all invarients, useful for public facing functions:
    // if invarient holds before function call, it should hold after.
    fn check_invarients(&self) -> bool {
        if self.incomming.len() != self.outgoing.len() {
            println!("INVARIENT_VIOLATION(graph.incomming.len()!=graph.incomming.len())\ngraph.incomming: {:?}\ngraph.outgoing {:?}", self.incomming, self.outgoing);
            false
        } else {
            true
        }
    }
}

impl<T> Default for DirectedGraph<T>
where
    T: Eq + Hash + Copy + std::fmt::Debug,
{
    fn default() -> Self {
        let out = Self {
            incomming: HashMap::new(),
            outgoing: HashMap::new(),
        };
        
        assert!(out.check_invarients());
        out
    }
}

impl<T> IntoIterator for DirectedGraph<T>
where 
    T: Eq + Hash + Copy + std::fmt::Debug
{
    type Item = T;
    type IntoIter = DirectedGraphIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        assert!(self.check_invarients());
        
        Self::IntoIter { 
            graph: self,
        }
    }
}

impl<T> Clone for DirectedGraph<T>
where 
    T: Eq + Hash + Copy + std::fmt::Debug
{
    fn clone(&self) -> Self {
        assert!(self.check_invarients());

        Self {
            incomming: self.incomming.clone(),
            outgoing: self.outgoing.clone(),
        }
    }
}


// for the iterator, next() should return a function which is not calling other internal functions
// we ensure only internal function calls are being tracked, such that no internal function calls means no function calls at all 
// thus, next() should fetch the next function which has an empty set out outgoing edges
pub struct DirectedGraphIter<T> 
where 
    T: Eq + Hash + Copy + std::fmt::Debug
{
    graph: DirectedGraph<T>,
}

impl<T> Iterator for DirectedGraphIter<T> 
where 
    T: Eq + Hash + Copy + std::fmt::Debug
{
    type Item = T;

    // each time next() find object with no outgoing-edges and remove it and its presence from the graph
    // if there are entries but none have 0-outgoing, there is a cycle! -> report + break the cycle
    fn next(&mut self) -> Option<Self::Item> {
        assert!(self.graph.check_invarients());

        if self.graph.incomming.is_empty() { return None } // all items parsed

        // find first node with no outgoing edges (next in topological ordering)
        for (from_node, to_node_ls) in &self.graph.outgoing {
            if to_node_ls.is_empty() {
                let from_node = from_node.clone();
                self.graph.remove(&from_node);

                assert!(self.graph.check_invarients());
                return Some(from_node);
            }
        }

        // There are more nodes but none have 0-outgoing, implies cycle!
        // Walk the graph until we encounter node twice, return said node
        let mut visitted: HashSet<T> = HashSet::with_capacity(self.graph.incomming.len());
        let mut node = self.graph.incomming.keys().next().unwrap(); // graph is non-empty
        loop {
            if visitted.insert(node.clone()) {
                // node had not yet been visited before
                // there is no node without outgoing edges so this should never fail
                node = self.graph.outgoing.get(node).unwrap().iter().next().unwrap(); 
            } else {
                // node had already been visited, it is part of a cycle, return it
                let node = node.clone();
                self.graph.remove(&node);
                
                assert!(self.graph.check_invarients());
                return Some(node);
            }
        }
    }
}