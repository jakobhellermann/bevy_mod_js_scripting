#![allow(dead_code)]
use std::cell::UnsafeCell;

use bevy::ecs as bevy_ecs;

use bevy_ecs::archetype::{
    Archetype, ArchetypeComponentId, ArchetypeGeneration, ArchetypeId, Archetypes,
};
use bevy_ecs::component::{ComponentId, ComponentTicks, StorageType};
use bevy_ecs::prelude::*;
use bevy_ecs::ptr::{Ptr, PtrMut, ThinSlicePtr, UnsafeCellDeref};
use bevy_ecs::query::{Access, FilteredAccess};
use bevy_ecs::storage::{ComponentSparseSet, Table, TableId, Tables};
use bevy_ecs::world::WorldId;
use fixedbitset::FixedBitSet;

#[derive(Clone, Copy, Debug)]
pub enum FetchKind {
    Ref(ComponentId),
    RefMut(ComponentId),
}

#[derive(Clone, Copy, Debug)]
pub enum FilterKind {
    With(ComponentId),
    Without(ComponentId),
    Changed(ComponentId),
    Added(ComponentId),
}

pub enum FetchResult<'w> {
    Ref(Ptr<'w>),
    RefMut {
        value: PtrMut<'w>,
        ticks: &'w mut ComponentTicks,
        last_change_tick: u32,
        change_tick: u32,
    },
}

impl FetchKind {
    fn component_id(self) -> ComponentId {
        match self {
            FetchKind::Ref(id) | FetchKind::RefMut(id) => id,
        }
    }

    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        match *self {
            FetchKind::Ref(component_id) | FetchKind::RefMut(component_id) => {
                archetype.contains(component_id)
            }
        }
    }

    fn update_archetype_component_access(
        &self,
        archetype: &Archetype,
        access: &mut Access<ArchetypeComponentId>,
    ) {
        match *self {
            FetchKind::Ref(id) => {
                access.add_read(archetype.get_archetype_component_id(id).unwrap())
            }
            FetchKind::RefMut(id) => {
                access.add_write(archetype.get_archetype_component_id(id).unwrap())
            }
        }
    }

    fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
        match *self {
            FetchKind::Ref(id) => {
                assert!(!access.access().has_write(id),"&{:?} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.", id);
                access.add_read(id);
            }
            FetchKind::RefMut(id) => {
                assert!(
                    !access.access().has_read(id),
                    "&mut {:?} conflicts with a previous access in this query. Mutable component access must be unique.",
                    id,
                );
                access.add_write(id)
            }
        }
    }
}

impl FilterKind {
    fn component_id(self) -> ComponentId {
        match self {
            FilterKind::With(id)
            | FilterKind::Without(id)
            | FilterKind::Changed(id)
            | FilterKind::Added(id) => id,
        }
    }

    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        match *self {
            FilterKind::With(id) => archetype.contains(id),
            FilterKind::Without(id) => !archetype.contains(id),
            FilterKind::Changed(id) => archetype.contains(id),
            FilterKind::Added(id) => archetype.contains(id),
        }
    }

    fn update_archetype_component_access(
        &self,
        archetype: &Archetype,
        access: &mut Access<ArchetypeComponentId>,
    ) {
        match *self {
            FilterKind::With(_) => {}
            FilterKind::Without(_) => {}
            FilterKind::Changed(id) | FilterKind::Added(id) => {
                access.add_read(archetype.get_archetype_component_id(id).unwrap())
            }
        }
    }

    fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
        match *self {
            FilterKind::With(id) => access.add_with(id),
            FilterKind::Without(id) => access.add_without(id),
            FilterKind::Changed(id) => {
                if access.access().has_write(id) {
                    panic!("With<{:?}> conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.", id);
                }
                access.add_read(id);
            }
            FilterKind::Added(id) => {
                if access.access().has_write(id) {
                    panic!("Added<{:?}> conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.", id);
                }
                access.add_read(id);
            }
        }
    }
}

struct ComponentFetchState<'w> {
    table_components: Option<Ptr<'w>>,
    entity_table_rows: Option<ThinSlicePtr<'w, usize>>,
    sparse_set: Option<&'w ComponentSparseSet>,

    table_ticks: Option<ThinSlicePtr<'w, UnsafeCell<ComponentTicks>>>,
    last_change_tick: u32,
    change_tick: u32,

    fetch_kind: FetchKind,
    storage_type: StorageType,
    component_size: usize,
}

impl<'w> ComponentFetchState<'w> {
    unsafe fn init(
        world: &'w World,
        last_change_tick: u32,
        change_tick: u32,
        fetch_kind: FetchKind,
    ) -> ComponentFetchState<'w> {
        let component_id = fetch_kind.component_id();
        let component_info = world.components().get_info(component_id).unwrap();

        ComponentFetchState {
            table_components: None,
            entity_table_rows: None,
            sparse_set: None,
            table_ticks: None,
            last_change_tick,
            change_tick,
            fetch_kind,
            storage_type: component_info.storage_type(),
            component_size: component_info.layout().size(),
        }
    }

    #[inline]
    unsafe fn set_archetype(&mut self, archetype: &'w Archetype, tables: &'w Tables) {
        match self.storage_type {
            StorageType::Table => {
                self.entity_table_rows = Some(archetype.entity_table_rows().into());
                let column = tables[archetype.table_id()]
                    .get_column(self.fetch_kind.component_id())
                    .unwrap();
                self.table_components = Some(column.get_data_ptr());
                self.table_ticks = Some(column.get_ticks_slice().into());
            }
            StorageType::SparseSet => {}
        }
    }

    #[inline]
    unsafe fn set_table(&mut self, table: &'w Table) {
        let column = table.get_column(self.fetch_kind.component_id()).unwrap();
        self.table_components = Some(column.get_data_ptr());
        self.table_ticks = Some(column.get_ticks_slice().into());
    }

    #[inline]
    unsafe fn archetype_fetch(
        &mut self,
        entity: Entity,
        archetype_index: usize,
    ) -> FetchResult<'w> {
        match self.storage_type {
            StorageType::Table => {
                let (entity_table_rows, (table_components, table_ticks)) = self
                    .entity_table_rows
                    .zip(self.table_components.zip(self.table_ticks))
                    .unwrap();
                let table_row = *entity_table_rows.get(archetype_index);
                let value = table_components.byte_add(table_row * self.component_size);

                match self.fetch_kind {
                    FetchKind::Ref(_) => FetchResult::Ref(value),
                    FetchKind::RefMut(_) => {
                        let component_ticks = table_ticks.get(table_row).deref_mut();
                        FetchResult::RefMut {
                            value: value.assert_unique(),
                            ticks: component_ticks,
                            last_change_tick: self.last_change_tick,
                            change_tick: self.change_tick,
                        }
                    }
                }
            }
            StorageType::SparseSet => {
                let sparse_set = self.sparse_set.unwrap();
                let (value, component_ticks) = sparse_set.get_with_ticks(entity).unwrap();
                match self.fetch_kind {
                    FetchKind::Ref(_) => FetchResult::Ref(value),
                    FetchKind::RefMut(_) => FetchResult::RefMut {
                        value: value.assert_unique(),
                        ticks: component_ticks.deref_mut(),
                        last_change_tick: self.last_change_tick,
                        change_tick: self.change_tick,
                    },
                }
            }
        }
    }

    #[inline]
    unsafe fn table_fetch(&mut self, table_row: usize) -> FetchResult<'w> {
        let (table_components, table_ticks) = self.table_components.zip(self.table_ticks).unwrap();
        let value = table_components.byte_add(table_row * self.component_size);
        let component_ticks = table_ticks.get(table_row);
        match self.fetch_kind {
            FetchKind::Ref(_) => FetchResult::Ref(value),
            FetchKind::RefMut(_) => FetchResult::RefMut {
                value: value.assert_unique(),
                ticks: component_ticks.deref_mut(),
                last_change_tick: self.last_change_tick,
                change_tick: self.change_tick,
            },
        }
    }
}

pub struct ComponentFetchStates<'w> {
    entities: Option<ThinSlicePtr<'w, Entity>>,
    components: Vec<ComponentFetchState<'w>>,
}

impl<'w> ComponentFetchStates<'w> {
    fn is_dense(&self) -> bool {
        self.components
            .iter()
            .all(|component| component.storage_type == StorageType::Table)
    }
}

impl<'w> ComponentFetchStates<'w> {
    unsafe fn init(
        world: &'w World,
        last_change_tick: u32,
        change_tick: u32,
        component_fetches: &[FetchKind],
    ) -> ComponentFetchStates<'w> {
        ComponentFetchStates {
            entities: None,
            components: component_fetches
                .iter()
                .map(|kind| ComponentFetchState::init(world, last_change_tick, change_tick, *kind))
                .collect(),
        }
    }

    #[inline]
    unsafe fn set_archetype(&mut self, archetype: &'w Archetype, tables: &'w Tables) {
        self.entities = Some(archetype.entities().into());
        self.components
            .iter_mut()
            .for_each(|component| component.set_archetype(archetype, tables));
    }

    #[inline]
    unsafe fn set_table(&mut self, table: &'w Table) {
        self.entities = Some(table.entities().into());
        self.components
            .iter_mut()
            .for_each(|component| component.set_table(table));
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, archetype_index: usize) -> Vec<FetchResult<'w>> {
        let entity = self.entity(archetype_index);
        self.components
            .iter_mut()
            .map(|component| component.archetype_fetch(entity, archetype_index))
            .collect()
    }

    #[inline]
    unsafe fn table_fetch(&mut self, table_row: usize) -> Vec<FetchResult<'w>> {
        self.components
            .iter_mut()
            .map(|component| component.table_fetch(table_row))
            .collect()
    }

    #[inline]
    unsafe fn entity(&mut self, index: usize) -> Entity {
        *self.entities.unwrap().get(index)
    }
}

struct ComponentFilterChangeDetection<'w> {
    table_ticks: Option<ThinSlicePtr<'w, UnsafeCell<ComponentTicks>>>,
    entity_table_rows: Option<ThinSlicePtr<'w, usize>>,
    entities: Option<ThinSlicePtr<'w, Entity>>,
    sparse_set: Option<&'w ComponentSparseSet>,
    last_change_tick: u32,
    change_tick: u32,
}

impl<'w> ComponentFilterChangeDetection<'w> {
    unsafe fn archetype_ticks(
        &self,
        storage_type: StorageType,
        archetype_index: usize,
    ) -> ComponentTicks {
        match storage_type {
            StorageType::Table => {
                let table_row = *self.entity_table_rows.unwrap().get(archetype_index);
                let ticks = *self.table_ticks.unwrap().get(table_row).deref();
                ticks
            }
            StorageType::SparseSet => {
                let entity = *self.entities.unwrap().get(archetype_index);
                let ticks = self
                    .sparse_set
                    .unwrap()
                    .get_ticks(entity)
                    .map(|ticks| &*ticks.get())
                    .cloned()
                    .unwrap();
                ticks
            }
        }
    }
}

struct ComponentFilterState<'w> {
    component_id: ComponentId,
    storage_type: StorageType,
    kind: FilterKind,
    change_detection: ComponentFilterChangeDetection<'w>,
}
impl<'w> ComponentFilterState<'w> {
    fn init(
        world: &'w World,
        last_change_tick: u32,
        change_tick: u32,
        kind: FilterKind,
    ) -> ComponentFilterState<'w> {
        let component_id = kind.component_id();
        let component_info = world.components().get_info(component_id).unwrap();
        ComponentFilterState {
            component_id,
            storage_type: component_info.storage_type(),
            kind,
            change_detection: ComponentFilterChangeDetection {
                table_ticks: None,
                entity_table_rows: None,
                entities: None,
                sparse_set: (component_info.storage_type() == StorageType::SparseSet)
                    .then(|| world.storages().sparse_sets.get(component_id).unwrap()),
                last_change_tick,
                change_tick,
            },
        }
    }

    #[inline]
    unsafe fn set_archetype(&mut self, archetype: &'w Archetype, tables: &'w Tables) {
        match self.kind {
            FilterKind::With(_) | FilterKind::Without(_) => {}
            FilterKind::Changed(_) | FilterKind::Added(_) => match self.storage_type {
                StorageType::Table => {
                    self.change_detection.entity_table_rows =
                        Some(archetype.entity_table_rows().into());
                    let table = &tables[archetype.table_id()];
                    self.change_detection.table_ticks = Some(
                        table
                            .get_column(self.component_id)
                            .unwrap()
                            .get_ticks_slice()
                            .into(),
                    );
                }
                StorageType::SparseSet => {
                    self.change_detection.entities = Some(archetype.entities().into())
                }
            },
        }
    }

    #[inline]
    unsafe fn set_table(&mut self, table: &'w Table) {
        match self.kind {
            FilterKind::With(_) | FilterKind::Without(_) => {}
            FilterKind::Changed(_) | FilterKind::Added(_) => {
                self.change_detection.table_ticks = Some(
                    table
                        .get_column(self.component_id)
                        .unwrap()
                        .get_ticks_slice()
                        .into(),
                );
            }
        }
    }

    #[inline]
    unsafe fn archetype_filter_fetch(&mut self, archetype_index: usize) -> bool {
        match self.kind {
            FilterKind::With(_) | FilterKind::Without(_) => true,
            FilterKind::Changed(_) => {
                let ticks = self
                    .change_detection
                    .archetype_ticks(self.storage_type, archetype_index);
                ticks.is_changed(
                    self.change_detection.last_change_tick,
                    self.change_detection.change_tick,
                )
            }
            FilterKind::Added(_) => {
                let ticks = self
                    .change_detection
                    .archetype_ticks(self.storage_type, archetype_index);
                ticks.is_added(
                    self.change_detection.last_change_tick,
                    self.change_detection.change_tick,
                )
            }
        }
    }

    #[inline]
    unsafe fn table_filter_fetch(&mut self, table_row: usize) -> bool {
        match self.kind {
            FilterKind::With(_) | FilterKind::Without(_) => true,
            FilterKind::Changed(_) => ComponentTicks::is_changed(
                &*(self.change_detection.table_ticks.unwrap().get(table_row)).deref(),
                self.change_detection.last_change_tick,
                self.change_detection.change_tick,
            ),
            FilterKind::Added(_) => ComponentTicks::is_added(
                &*(self.change_detection.table_ticks.unwrap().get(table_row)).deref(),
                self.change_detection.last_change_tick,
                self.change_detection.change_tick,
            ),
        }
    }
}

pub struct ComponentFilterStates<'w> {
    filters: Vec<ComponentFilterState<'w>>,
}

impl<'w> ComponentFilterStates<'w> {
    fn is_dense(&self) -> bool {
        self.filters
            .iter()
            .all(|filter| filter.storage_type == StorageType::Table)
    }
}

impl<'w> ComponentFilterStates<'w> {
    unsafe fn init(
        world: &'w World,
        last_change_tick: u32,
        change_tick: u32,
        filters: &[FilterKind],
    ) -> ComponentFilterStates<'w> {
        ComponentFilterStates {
            filters: filters
                .iter()
                .map(|kind| ComponentFilterState::init(world, last_change_tick, change_tick, *kind))
                .collect(),
        }
    }

    #[inline]
    unsafe fn set_archetype(&mut self, archetype: &'w Archetype, tables: &'w Tables) {
        self.filters
            .iter_mut()
            .for_each(|filter| filter.set_archetype(archetype, tables));
    }

    #[inline]
    unsafe fn set_table(&mut self, table: &'w Table) {
        self.filters
            .iter_mut()
            .for_each(|filter| filter.set_table(table));
    }

    #[inline]
    unsafe fn archetype_filter_fetch(&mut self, archetype_index: usize) -> bool {
        self.filters
            .iter_mut()
            .all(|filter| filter.archetype_filter_fetch(archetype_index))
    }

    #[inline]
    unsafe fn table_filter_fetch(&mut self, table_row: usize) -> bool {
        self.filters
            .iter_mut()
            .all(|filter| filter.table_filter_fetch(table_row))
    }
}

pub struct DynamicQuery {
    world_id: WorldId,
    component_fetches: Vec<FetchKind>,
    filters: Vec<FilterKind>,

    archetype_generation: ArchetypeGeneration,
    // NOTE: we maintain both a TableId bitset and a vec because iterating the vec is faster
    matched_tables: FixedBitSet,
    matched_table_ids: Vec<TableId>,
    // NOTE: we maintain both a ArchetypeId bitset and a vec because iterating the vec is faster
    matched_archetypes: FixedBitSet,
    matched_archetype_ids: Vec<ArchetypeId>,
    #[allow(unused)]
    component_access: FilteredAccess<ComponentId>,
    archetype_component_access: Access<ArchetypeComponentId>,
}

impl DynamicQuery {
    pub fn new(world: &World, component_fetches: Vec<FetchKind>, filters: Vec<FilterKind>) -> Self {
        let mut component_access = FilteredAccess::default();
        component_fetches
            .iter()
            .for_each(|fetch| fetch.update_component_access(&mut component_access));

        // Use a temporary empty FilteredAccess for filters. This prevents them from conflicting with the
        // main Query's `fetch_state` access. Filters are allowed to conflict with the main query fetch
        // because they are evaluated *before* a specific reference is constructed.
        let mut filter_component_access = FilteredAccess::default();
        filters
            .iter()
            .for_each(|filter| filter.update_component_access(&mut filter_component_access));

        // Merge the temporary filter access with the main access. This ensures that filter access is
        // properly considered in a global "cross-query" context (both within systems and across systems).
        component_access.extend(&filter_component_access);

        let mut query = DynamicQuery {
            world_id: world.id(),
            component_fetches,
            filters,
            component_access,
            archetype_generation: ArchetypeGeneration::initial(),
            matched_tables: Default::default(),
            matched_table_ids: Vec::new(),
            matched_archetypes: Default::default(),
            matched_archetype_ids: Vec::new(),
            archetype_component_access: Default::default(),
        };
        query.update_archetypes(world);
        query
    }

    fn validate_world(&self, world: &World) {
        assert!(
            world.id() == self.world_id,
            "Attempted to use {} with a mismatched World. QueryStates can only be used with the World they were created from.",
                std::any::type_name::<Self>(),
        );
    }

    pub fn update_archetypes(&mut self, world: &World) {
        self.validate_world(world);
        let archetypes = world.archetypes();
        let new_generation = archetypes.generation();
        let old_generation = std::mem::replace(&mut self.archetype_generation, new_generation);
        let archetype_index_range = old_generation.value()..new_generation.value();

        for archetype_index in archetype_index_range {
            self.new_archetype(&archetypes[ArchetypeId::new(archetype_index)]);
        }
    }

    fn new_archetype(&mut self, archetype: &Archetype) {
        if self
            .component_fetches
            .iter()
            .all(|f| f.matches_archetype(archetype))
            && self.filters.iter().all(|f| f.matches_archetype(archetype))
        {
            self.component_fetches.iter().for_each(|s| {
                s.update_archetype_component_access(archetype, &mut self.archetype_component_access)
            });
            self.filters.iter().for_each(|s| {
                s.update_archetype_component_access(archetype, &mut self.archetype_component_access)
            });
            let archetype_index = archetype.id().index();
            if !self.matched_archetypes.contains(archetype_index) {
                self.matched_archetypes.grow(archetype_index + 1);
                self.matched_archetypes.set(archetype_index, true);
                self.matched_archetype_ids.push(archetype.id());
            }
            let table_index = archetype.table_id().index();
            if !self.matched_tables.contains(table_index) {
                self.matched_tables.grow(table_index + 1);
                self.matched_tables.set(table_index, true);
                self.matched_table_ids.push(archetype.table_id());
            }
        }
    }

    pub fn iter_mut<'w, 's>(&'s mut self, world: &'w mut World) -> DynamicQueryIter<'w, 's> {
        self.update_archetypes(world);
        // SAFETY: query has unique world access
        unsafe {
            self.iter_unchecked_manual(world, world.last_change_tick(), world.read_change_tick())
        }
    }

    pub unsafe fn iter_unchecked_manual<'w, 's>(
        &'s self,
        world: &'w World,
        last_change_tick: u32,
        change_tick: u32,
    ) -> DynamicQueryIter<'w, 's> {
        DynamicQueryIter::new(world, self, last_change_tick, change_tick)
    }
}

pub struct DynamicQueryIter<'w, 's> {
    tables: &'w Tables,
    archetypes: &'w Archetypes,
    is_dense: bool,
    fetch: ComponentFetchStates<'w>,
    filter: ComponentFilterStates<'w>,
    table_id_iter: std::slice::Iter<'s, TableId>,
    archetype_id_iter: std::slice::Iter<'s, ArchetypeId>,
    current_len: usize,
    current_index: usize,
}
impl<'w, 's> DynamicQueryIter<'w, 's> {
    fn new(
        world: &'w World,
        query: &'s DynamicQuery,
        last_change_tick: u32,
        change_tick: u32,
    ) -> DynamicQueryIter<'w, 's> {
        let fetch = unsafe {
            ComponentFetchStates::init(
                world,
                last_change_tick,
                change_tick,
                &query.component_fetches,
            )
        };
        let filter = unsafe {
            ComponentFilterStates::init(world, last_change_tick, change_tick, &query.filters)
        };

        let is_dense = fetch.is_dense() && filter.is_dense();

        DynamicQueryIter {
            tables: &world.storages().tables,
            archetypes: world.archetypes(),
            is_dense,
            fetch,
            filter,
            table_id_iter: query.matched_table_ids.iter(),
            archetype_id_iter: query.matched_archetype_ids.iter(),
            current_len: 0,
            current_index: 0,
        }
    }
}

pub struct DynamicQueryItem<'w> {
    pub entity: Entity,
    pub items: Vec<FetchResult<'w>>,
}

impl<'w, 's> Iterator for DynamicQueryIter<'w, 's> {
    type Item = DynamicQueryItem<'w>;

    fn next(&mut self) -> Option<DynamicQueryItem<'w>> {
        unsafe {
            if self.is_dense {
                loop {
                    if self.current_index == self.current_len {
                        let table_id = self.table_id_iter.next()?;
                        let table = &self.tables[*table_id];
                        self.fetch.set_table(table);
                        self.filter.set_table(table);
                        self.current_len = table.len();
                        self.current_index = 0;
                        continue;
                    }

                    if !self.filter.table_filter_fetch(self.current_index) {
                        self.current_index += 1;
                        continue;
                    }

                    let entity = self.fetch.entity(self.current_index);
                    let items = self.fetch.table_fetch(self.current_index);

                    self.current_index += 1;

                    return Some(DynamicQueryItem { entity, items });
                }
            } else {
                loop {
                    if self.current_index == self.current_len {
                        let archetype_id = self.archetype_id_iter.next()?;
                        let archetype = &self.archetypes[*archetype_id];
                        self.fetch.set_archetype(archetype, self.tables);
                        self.filter.set_archetype(archetype, self.tables);
                        self.current_len = archetype.len();
                        self.current_index = 0;
                        continue;
                    }

                    if !self.filter.archetype_filter_fetch(self.current_index) {
                        self.current_index += 1;
                        continue;
                    }

                    let entity = self.fetch.entity(self.current_index);
                    let items = self.fetch.archetype_fetch(self.current_index);

                    self.current_index += 1;

                    return Some(DynamicQueryItem { entity, items });
                }
            }
        }
    }
}
