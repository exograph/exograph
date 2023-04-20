// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::mapped_arena::MappedArena;

use crate::typechecker::Type;

use super::system_builder::SystemContextBuilding;

pub(crate) fn build_primitives(types: &MappedArena<Type>, building: &mut SystemContextBuilding) {
    for (_, typ) in types.iter() {
        if let Type::Primitive(pt) = typ {
            let name = pt.name();

            building.primitive_types.add(&name, pt.clone());
        }
    }
}
