// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

export function addAndDouble(i, j) {
  return (i+j) * 2;
}

  // TODO: avoid fetching from a server in tests
export async function getJson(id) {
  const r = await fetch("https://jsonplaceholder.typicode.com/todos/" + id);
  return await r.json();
}


