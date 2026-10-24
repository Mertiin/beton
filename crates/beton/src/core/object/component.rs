//! Sparse-set component storage and tuple queries.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};

use super::game_object::{GameObject, GameObjectId};
use super::world::World;

const ABSENT: u32 = u32::MAX;

/// One typed sparse set: dense values packed by type, sparse index by object slot.
pub(super) struct ComponentStore<T> {
    dense: Vec<T>,
    owners: Vec<u32>,
    sparse: Vec<u32>,
}

impl<T> Default for ComponentStore<T> {
    fn default() -> Self {
        Self {
            dense: Vec::new(),
            owners: Vec::new(),
            sparse: Vec::new(),
        }
    }
}

impl<T> ComponentStore<T> {
    fn ensure_sparse(&mut self, index: u32) {
        let need = index as usize + 1;
        if self.sparse.len() < need {
            self.sparse.resize(need, ABSENT);
        }
    }

    pub(super) fn insert(&mut self, index: u32, value: T) -> Option<T> {
        self.ensure_sparse(index);
        let slot = self.sparse[index as usize];
        if slot != ABSENT {
            return Some(std::mem::replace(&mut self.dense[slot as usize], value));
        }
        let dense_index = self.dense.len() as u32;
        self.dense.push(value);
        self.owners.push(index);
        self.sparse[index as usize] = dense_index;
        None
    }

    pub(super) fn get(&self, index: u32) -> Option<&T> {
        let dense = *self.sparse.get(index as usize)?;
        if dense == ABSENT {
            return None;
        }
        self.dense.get(dense as usize)
    }

    pub(super) fn get_mut(&mut self, index: u32) -> Option<&mut T> {
        let dense = *self.sparse.get(index as usize)?;
        if dense == ABSENT {
            return None;
        }
        self.dense.get_mut(dense as usize)
    }

    pub(super) fn remove(&mut self, index: u32) -> Option<T> {
        let dense = *self.sparse.get(index as usize)?;
        if dense == ABSENT {
            return None;
        }
        let dense = dense as usize;
        let removed = self.dense.swap_remove(dense);
        self.owners.swap_remove(dense);
        self.sparse[index as usize] = ABSENT;
        if dense < self.dense.len() {
            let moved_owner = self.owners[dense];
            self.ensure_sparse(moved_owner);
            self.sparse[moved_owner as usize] = dense as u32;
        }
        Some(removed)
    }

    pub(super) fn contains(&self, index: u32) -> bool {
        matches!(self.sparse.get(index as usize), Some(&d) if d != ABSENT)
    }

    pub(super) fn len(&self) -> usize {
        self.dense.len()
    }

    pub(super) fn owners(&self) -> &[u32] {
        &self.owners
    }

    fn clear_all(&mut self) {
        self.dense.clear();
        self.owners.clear();
        self.sparse.clear();
    }
}

pub(super) trait ErasedStore: Any {
    fn clear_slot(&mut self, index: u32);
    fn clear_all(&mut self);
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: 'static> ErasedStore for ComponentStore<T> {
    fn clear_slot(&mut self, index: u32) {
        let _ = self.remove(index);
    }

    fn clear_all(&mut self) {
        ComponentStore::clear_all(self);
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Map of type → sparse set, owned by [`World`].
#[derive(Default)]
pub(super) struct ComponentMap {
    stores: HashMap<TypeId, Box<dyn ErasedStore>>,
}

impl ComponentMap {
    pub(super) fn insert<T: 'static>(&mut self, index: u32, value: T) -> Option<T> {
        let store = self
            .stores
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(ComponentStore::<T>::default()));
        store
            .as_any_mut()
            .downcast_mut::<ComponentStore<T>>()
            .expect("typed store")
            .insert(index, value)
    }

    pub(super) fn get<T: 'static>(&self, index: u32) -> Option<&T> {
        self.stores
            .get(&TypeId::of::<T>())?
            .as_any()
            .downcast_ref::<ComponentStore<T>>()?
            .get(index)
    }

    pub(super) fn get_mut<T: 'static>(&mut self, index: u32) -> Option<&mut T> {
        self.stores
            .get_mut(&TypeId::of::<T>())?
            .as_any_mut()
            .downcast_mut::<ComponentStore<T>>()?
            .get_mut(index)
    }

    pub(super) fn remove<T: 'static>(&mut self, index: u32) -> Option<T> {
        self.stores
            .get_mut(&TypeId::of::<T>())?
            .as_any_mut()
            .downcast_mut::<ComponentStore<T>>()?
            .remove(index)
    }

    pub(super) fn contains<T: 'static>(&self, index: u32) -> bool {
        self.stores
            .get(&TypeId::of::<T>())
            .and_then(|s| s.as_any().downcast_ref::<ComponentStore<T>>())
            .is_some_and(|s| s.contains(index))
    }

    pub(super) fn count<T: 'static>(&self) -> usize {
        self.stores
            .get(&TypeId::of::<T>())
            .and_then(|s| s.as_any().downcast_ref::<ComponentStore<T>>())
            .map(|s| s.len())
            .unwrap_or(0)
    }

    pub(super) fn clear_slot(&mut self, index: u32) {
        for store in self.stores.values_mut() {
            store.clear_slot(index);
        }
    }

    pub(super) fn clear_all(&mut self) {
        for store in self.stores.values_mut() {
            store.clear_all();
        }
        self.stores.clear();
    }

    pub(super) fn take_store<T: 'static>(&mut self) -> Option<Box<ComponentStore<T>>> {
        let erased = self.stores.remove(&TypeId::of::<T>())?;
        erased.into_any().downcast::<ComponentStore<T>>().ok()
    }

    pub(super) fn put_store<T: 'static>(&mut self, store: Box<ComponentStore<T>>) {
        self.stores.insert(TypeId::of::<T>(), store);
    }
}

/// Types that can be borrowed together in [`World::each`].
pub trait Query {
    /// Mutable borrows produced for one matching object.
    type Item<'a>;

    #[doc(hidden)]
    fn run<F>(world: &mut World, f: F)
    where
        F: for<'a> FnMut(GameObjectId, &'a mut GameObject, Self::Item<'a>);
}

fn assert_unique(ids: &[TypeId]) {
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            assert!(
                ids[i] != ids[j],
                "Query lists the same component type more than once"
            );
        }
    }
}

/// Puts `value` back into `store` on drop, including after a panic inside `each`.
struct Restore<'a, T> {
    store: &'a mut ComponentStore<T>,
    index: u32,
    value: Option<T>,
}

impl<'a, T> Restore<'a, T> {
    fn new(store: &'a mut ComponentStore<T>, index: u32, value: T) -> Self {
        Self {
            store,
            index,
            value: Some(value),
        }
    }

    fn value_mut(&mut self) -> &mut T {
        self.value.as_mut().expect("restore still holds value")
    }
}

impl<T> Drop for Restore<'_, T> {
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            self.store.insert(self.index, value);
        }
    }
}

impl<A: 'static> Query for (A,) {
    type Item<'a> = (&'a mut A,);

    fn run<F>(world: &mut World, mut f: F)
    where
        F: for<'a> FnMut(GameObjectId, &'a mut GameObject, Self::Item<'a>),
    {
        assert_unique(&[TypeId::of::<A>()]);
        let Some(mut store) = world.components.take_store::<A>() else {
            return;
        };
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            let owners = store.owners().to_vec();
            for index in owners {
                if !world.slot_is_live(index) || !store.contains(index) {
                    continue;
                }
                let id = GameObjectId(index, world.slot_generation(index));
                let Some(value) = store.remove(index) else {
                    continue;
                };
                let mut restore = Restore::new(&mut store, index, value);
                if let Some(object) = world.object_mut(index) {
                    f(id, object, (restore.value_mut(),));
                }
            }
        }));
        world.components.put_store(store);
        if let Err(payload) = result {
            panic::resume_unwind(payload);
        }
    }
}

impl<A: 'static, B: 'static> Query for (A, B) {
    type Item<'a> = (&'a mut A, &'a mut B);

    fn run<F>(world: &mut World, mut f: F)
    where
        F: for<'a> FnMut(GameObjectId, &'a mut GameObject, Self::Item<'a>),
    {
        assert_unique(&[TypeId::of::<A>(), TypeId::of::<B>()]);
        let mut store_a = world.components.take_store::<A>();
        let mut store_b = world.components.take_store::<B>();
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            let (Some(sa), Some(sb)) = (store_a.as_mut(), store_b.as_mut()) else {
                return;
            };
            let owners = if sa.len() <= sb.len() {
                sa.owners().to_vec()
            } else {
                sb.owners().to_vec()
            };
            for index in owners {
                if !world.slot_is_live(index) || !sa.contains(index) || !sb.contains(index) {
                    continue;
                }
                let id = GameObjectId(index, world.slot_generation(index));
                let Some(a) = sa.remove(index) else {
                    continue;
                };
                let Some(b) = sb.remove(index) else {
                    sa.insert(index, a);
                    continue;
                };
                let mut ra = Restore::new(sa, index, a);
                let mut rb = Restore::new(sb, index, b);
                if let Some(object) = world.object_mut(index) {
                    f(id, object, (ra.value_mut(), rb.value_mut()));
                }
            }
        }));
        if let Some(s) = store_a {
            world.components.put_store(s);
        }
        if let Some(s) = store_b {
            world.components.put_store(s);
        }
        if let Err(payload) = result {
            panic::resume_unwind(payload);
        }
    }
}

impl<A: 'static, B: 'static, C: 'static> Query for (A, B, C) {
    type Item<'a> = (&'a mut A, &'a mut B, &'a mut C);

    fn run<F>(world: &mut World, mut f: F)
    where
        F: for<'a> FnMut(GameObjectId, &'a mut GameObject, Self::Item<'a>),
    {
        assert_unique(&[TypeId::of::<A>(), TypeId::of::<B>(), TypeId::of::<C>()]);
        let mut store_a = world.components.take_store::<A>();
        let mut store_b = world.components.take_store::<B>();
        let mut store_c = world.components.take_store::<C>();
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            let (Some(sa), Some(sb), Some(sc)) =
                (store_a.as_mut(), store_b.as_mut(), store_c.as_mut())
            else {
                return;
            };
            let mut owners = sa.owners().to_vec();
            let mut best = sa.len();
            if sb.len() < best {
                best = sb.len();
                owners = sb.owners().to_vec();
            }
            if sc.len() < best {
                owners = sc.owners().to_vec();
            }
            for index in owners {
                if !world.slot_is_live(index)
                    || !sa.contains(index)
                    || !sb.contains(index)
                    || !sc.contains(index)
                {
                    continue;
                }
                let id = GameObjectId(index, world.slot_generation(index));
                let Some(a) = sa.remove(index) else {
                    continue;
                };
                let Some(b) = sb.remove(index) else {
                    sa.insert(index, a);
                    continue;
                };
                let Some(c) = sc.remove(index) else {
                    sa.insert(index, a);
                    sb.insert(index, b);
                    continue;
                };
                let mut ra = Restore::new(sa, index, a);
                let mut rb = Restore::new(sb, index, b);
                let mut rc = Restore::new(sc, index, c);
                if let Some(object) = world.object_mut(index) {
                    f(
                        id,
                        object,
                        (ra.value_mut(), rb.value_mut(), rc.value_mut()),
                    );
                }
            }
        }));
        if let Some(s) = store_a {
            world.components.put_store(s);
        }
        if let Some(s) = store_b {
            world.components.put_store(s);
        }
        if let Some(s) = store_c {
            world.components.put_store(s);
        }
        if let Err(payload) = result {
            panic::resume_unwind(payload);
        }
    }
}

impl<A: 'static, B: 'static, C: 'static, D: 'static> Query for (A, B, C, D) {
    type Item<'a> = (&'a mut A, &'a mut B, &'a mut C, &'a mut D);

    fn run<F>(world: &mut World, mut f: F)
    where
        F: for<'a> FnMut(GameObjectId, &'a mut GameObject, Self::Item<'a>),
    {
        assert_unique(&[
            TypeId::of::<A>(),
            TypeId::of::<B>(),
            TypeId::of::<C>(),
            TypeId::of::<D>(),
        ]);
        let mut store_a = world.components.take_store::<A>();
        let mut store_b = world.components.take_store::<B>();
        let mut store_c = world.components.take_store::<C>();
        let mut store_d = world.components.take_store::<D>();
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            let (Some(sa), Some(sb), Some(sc), Some(sd)) = (
                store_a.as_mut(),
                store_b.as_mut(),
                store_c.as_mut(),
                store_d.as_mut(),
            ) else {
                return;
            };
            let mut owners = sa.owners().to_vec();
            let mut best = sa.len();
            if sb.len() < best {
                best = sb.len();
                owners = sb.owners().to_vec();
            }
            if sc.len() < best {
                best = sc.len();
                owners = sc.owners().to_vec();
            }
            if sd.len() < best {
                owners = sd.owners().to_vec();
            }
            for index in owners {
                if !world.slot_is_live(index)
                    || !sa.contains(index)
                    || !sb.contains(index)
                    || !sc.contains(index)
                    || !sd.contains(index)
                {
                    continue;
                }
                let id = GameObjectId(index, world.slot_generation(index));
                let Some(a) = sa.remove(index) else {
                    continue;
                };
                let Some(b) = sb.remove(index) else {
                    sa.insert(index, a);
                    continue;
                };
                let Some(c) = sc.remove(index) else {
                    sa.insert(index, a);
                    sb.insert(index, b);
                    continue;
                };
                let Some(d) = sd.remove(index) else {
                    sa.insert(index, a);
                    sb.insert(index, b);
                    sc.insert(index, c);
                    continue;
                };
                let mut ra = Restore::new(sa, index, a);
                let mut rb = Restore::new(sb, index, b);
                let mut rc = Restore::new(sc, index, c);
                let mut rd = Restore::new(sd, index, d);
                if let Some(object) = world.object_mut(index) {
                    f(
                        id,
                        object,
                        (
                            ra.value_mut(),
                            rb.value_mut(),
                            rc.value_mut(),
                            rd.value_mut(),
                        ),
                    );
                }
            }
        }));
        if let Some(s) = store_a {
            world.components.put_store(s);
        }
        if let Some(s) = store_b {
            world.components.put_store(s);
        }
        if let Some(s) = store_c {
            world.components.put_store(s);
        }
        if let Some(s) = store_d {
            world.components.put_store(s);
        }
        if let Err(payload) = result {
            panic::resume_unwind(payload);
        }
    }
}
