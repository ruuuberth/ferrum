    // Web Storage. The backing maps live in the Rust runtime; this wrapper
    // exposes the `Storage` API and named properties (`localStorage.foo`).
    const makeStorage = (area) => {
        const api = {
            get length() {
                return __blitz_storage_keys(area).length;
            },
            key(index) {
                const keys = __blitz_storage_keys(area);
                const value = keys[index];
                return value === undefined ? null : value;
            },
            getItem(key) {
                const value = __blitz_storage_get(area, String(key));
                return value === null ? null : value;
            },
            setItem(key, value) {
                __blitz_storage_set(area, String(key), String(value));
            },
            removeItem(key) {
                __blitz_storage_remove(area, String(key));
            },
            clear() {
                __blitz_storage_clear(area);
            },
        };
        return new Proxy(api, {
            get(target, property, receiver) {
                if (property in target) {
                    return Reflect.get(target, property, receiver);
                }
                if (typeof property === "string") {
                    const value = __blitz_storage_get(area, property);
                    return value === null ? undefined : value;
                }
                return undefined;
            },
            set(target, property, value) {
                if (typeof property === "string" && !(property in target)) {
                    __blitz_storage_set(area, property, String(value));
                    return true;
                }
                return Reflect.set(target, property, value);
            },
            has(target, property) {
                if (property in target) {
                    return true;
                }
                return (
                    typeof property === "string" &&
                    __blitz_storage_get(area, property) !== null
                );
            },
            deleteProperty(target, property) {
                if (typeof property === "string" && !(property in target)) {
                    __blitz_storage_remove(area, property);
                    return true;
                }
                return Reflect.deleteProperty(target, property);
            },
            ownKeys(target) {
                return Array.from(
                    new Set([...Reflect.ownKeys(target), ...__blitz_storage_keys(area)])
                );
            },
            getOwnPropertyDescriptor(target, property) {
                if (property in target) {
                    return Reflect.getOwnPropertyDescriptor(target, property);
                }
                if (typeof property === "string") {
                    const value = __blitz_storage_get(area, property);
                    if (value !== null) {
                        return {
                            value,
                            writable: true,
                            enumerable: true,
                            configurable: true,
                        };
                    }
                }
                return undefined;
            },
        });
    };

    if (typeof globalThis.localStorage === "undefined") {
        globalThis.localStorage = makeStorage("local");
    }
    if (typeof globalThis.sessionStorage === "undefined") {
        globalThis.sessionStorage = makeStorage("session");
    }
