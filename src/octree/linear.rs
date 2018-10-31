use super::{
    morton::morton_levels, Folder, Gatherer, Morton, MortonMap, MortonRegion, MortonRegionMap,
    MortonWrapper,
};

/// An octree that starts with a cube from [-1, 1] in each dimension and will only expand.
#[derive(Clone)]
pub struct Linear<T, M> {
    /// The leaves of the octree. Uses `SmallVec` because in most cases this shouldn't have more than one element.
    leaves: MortonMap<T, M>,
    /// The each internal node either contains a `null` Morton or a non-null Morton which points to a leaf.
    /// Nodes which are not explicity stated implicitly indicate that it must be traversed deeper.
    internals: MortonRegionMap<M, M>,
}

impl<T, M> Default for Linear<T, M>
where
    M: Morton,
{
    fn default() -> Self {
        let mut internals = MortonRegionMap::default();
        internals.insert(MortonRegion::default(), M::null());
        Linear {
            leaves: MortonMap::<_, M>::default(),
            internals,
        }
    }
}

impl<T, M> Linear<T, M>
where
    M: Morton,
{
    pub fn new() -> Self {
        Default::default()
    }

    /// Inserts the item into the octree.
    ///
    /// If another element occupied the exact same morton, it will be evicted and replaced.
    pub fn insert(&mut self, morton: M, item: T) {
        use std::collections::hash_map::Entry::*;
        // First we must insert the node into the leaves.
        match self.leaves.entry(MortonWrapper(morton)) {
            Occupied(mut o) => {
                o.insert(item);
            }
            Vacant(v) => {
                v.insert(item);

                // Because it was vacant, we need to adjust the tree's internal nodes.
                for mut region in morton_levels(morton) {
                    // Check if the region is in the map.
                    if let Occupied(mut o) = self.internals.entry(region) {
                        // It was in the map. Check if it was null or not.
                        if o.get().is_null() {
                            // It was null, so just replace the null with the leaf.
                            *o.get_mut() = morton;
                            // Now return because we are done.
                            return;
                        } else {
                            // It was not null, so it is a leaf.
                            // This means that we need to move the leaf to its sub-region.
                            // We also need to populate the other 6 null nodes created by this operation.
                            let leaf = o.remove_entry().1;
                            // Keep making the tree deeper until both leaves differ.
                            // TODO: Some bittwiddling with mortons might be able to get the number of traversals.
                            for level in region.level..M::dim_bits() {
                                let leaf_level = leaf.get_level(level);
                                let item_level = morton.get_level(level);
                                if leaf_level == item_level {
                                    // They were the same so set every other region to null.
                                    for i in 0..8 {
                                        if i != leaf_level {
                                            self.internals.insert(region.enter(i), M::null());
                                        }
                                    }
                                    region = region.enter(leaf_level);
                                } else {
                                    // They were different, so set the other 6 regions null and make 2 leaves.
                                    for i in 0..8 {
                                        if i == leaf_level {
                                            self.internals.insert(region.enter(i), leaf);
                                        } else if i == item_level {
                                            self.internals.insert(region.enter(i), morton);
                                        } else {
                                            self.internals.insert(region.enter(i), M::null());
                                        }
                                    }
                                    // Now we must return as we have added the leaves.
                                    return;
                                }
                            }
                            unreachable!();
                        }
                    }
                }
            }
        }
    }

    /// This gathers the octree in a tree fold by gathering leaves with `gatherer` and folding with `folder`.
    pub fn iter_gather_deep_linear_hashed_tree_fold<G, F>(
        &self,
        region: MortonRegion<M>,
        gatherer: &G,
        folder: &F,
    ) -> MortonRegionMap<G::Sum, M>
    where
        G: Gatherer<T, M>,
        F: Folder<Sum = G::Sum>,
        G::Sum: Clone,
    {
        let mut map = MortonRegionMap::default();
        self.iter_gather_deep_linear_hashed_tree_fold_map_adder(region, gatherer, folder, &mut map);
        map
    }

    /// This gathers the octree in a tree fold by gathering leaves with `gatherer` and folding with `folder`.
    pub fn iter_gather_deep_linear_hashed_tree_fold_map_adder<G, F>(
        &self,
        region: MortonRegion<M>,
        gatherer: &G,
        folder: &F,
        map: &mut MortonRegionMap<G::Sum, M>,
    ) -> Option<G::Sum>
    where
        G: Gatherer<T, M>,
        F: Folder<Sum = G::Sum>,
        G::Sum: Clone,
    {
        match self.internals.get(&region) {
            Some(m) if !m.is_null() => {
                // This is a leaf node.
                let sum = gatherer
                    .gather(std::iter::once(&self.leaves[&MortonWrapper(*m)]).map(|i| (*m, i)));
                map.insert(region, sum.clone());
                Some(sum)
            }
            None => {
                // This needs to be traversed deeper.
                if let Some(sum) = folder.sum((0..8).filter_map(|i| {
                    self.iter_gather_deep_linear_hashed_tree_fold_map_adder(
                        region.enter(i),
                        gatherer,
                        folder,
                        map,
                    )
                })) {
                    map.insert(region, sum.clone());
                    Some(sum)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

impl<T, M> Extend<(M, T)> for Linear<T, M>
where
    M: Morton + Default,
{
    fn extend<I>(&mut self, it: I)
    where
        I: IntoIterator<Item = (M, T)>,
    {
        for (morton, item) in it.into_iter() {
            self.insert(morton, item);
        }
    }
}
