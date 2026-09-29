const fusedImports = {};

async function link(url, deps, initSync) {
  await Promise.all(deps.map((dep) => dep()));

  const response = await fetch(url);

  if (!response.ok) {
    throw new Error(`${response.status} ${response.statusText}`);
  }

  const main = initSync(undefined, undefined);
  const imports = {
    env: { memory: main.memory },
    __wasm_split: {
      __indirect_function_table: main.__indirect_function_table,
      __stack_pointer: main.__stack_pointer,
      __tls_base: main.__tls_base,
      memory: main.memory,
    },
  };

  for (const name in main) imports.__wasm_split[name] = main[name];
  for (const name in fusedImports) imports.__wasm_split[name] = fusedImports[name];

  const { instance } = await WebAssembly.instantiateStreaming(response, imports);

  for (const name in instance.exports) fusedImports[name] = instance.exports[name];
}

export function makeLoad(url, deps, initSync) {
  let loading = null;

  return (callbackIndex, callbackData) => {
    if (loading === null) {
      loading = link(url, deps, initSync);
      loading.catch(() => {
        loading = null;
      });
    }

    if (callbackIndex === undefined) return loading;

    const settle = (success) =>
      initSync(undefined, undefined).__indirect_function_table.get(callbackIndex)(
        callbackData,
        success,
      );

    loading.then(
      () => settle(true),
      (error) => {
        console.error("skene: a split module failed to load", url.href, error);
        settle(false);
      },
    );
  };
}
