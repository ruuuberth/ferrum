    // Browser capabilities used by modern sites for feature detection and
    // layout. Blitz has a single viewport, no printing and a touch-first
    // mobile shell, which is what these stubs report.

    const parseCssLength = (value) => {
        const match = String(value).trim().match(/^(-?[0-9.]+)(px|em|rem)?$/);
        if (!match) {
            return NaN;
        }
        const number = Number(match[1]);
        const unit = match[2] || "px";
        return unit === "em" || unit === "rem" ? number * 16 : number;
    };

    const evaluateMediaFeature = (name, value) => {
        const width = Number(window.innerWidth) || 0;
        const height = Number(window.innerHeight) || 0;
        const length = parseCssLength(value);
        const touch = Number(navigator.maxTouchPoints) > 0;
        switch (name) {
            case "width":
            case "device-width":
                return width === length;
            case "height":
            case "device-height":
                return height === length;
            case "min-width":
            case "min-device-width":
                return width >= length;
            case "max-width":
            case "max-device-width":
                return width <= length;
            case "min-height":
            case "min-device-height":
                return height >= length;
            case "max-height":
            case "max-device-height":
                return height <= length;
            case "orientation":
                return value === (height >= width ? "portrait" : "landscape");
            case "prefers-color-scheme":
                // Blitz renders a light page by default.
                return value === "light" || value === "no-preference";
            case "prefers-reduced-motion":
                return value === "no-preference";
            case "prefers-contrast":
                return value === "no-preference";
            case "pointer":
            case "any-pointer":
                return value === (touch ? "coarse" : "fine");
            case "hover":
            case "any-hover":
                return value === (touch ? "none" : "hover");
            case "display-mode":
                return value === "browser";
            case "scripting":
                return value === "enabled";
            case "color":
                return value !== "";
            case "monochrome":
                return value === "0";
            case "resolution":
            case "min-resolution":
            case "max-resolution":
                return false;
            default:
                return false;
        }
    };

    const evaluateMediaQueryPart = (part) => {
        part = part.trim().toLowerCase();
        if (part === "" || part === "all" || part === "screen") {
            return true;
        }
        if (part === "print" || part === "speech") {
            return false;
        }
        let negate = false;
        if (part.startsWith("not ")) {
            negate = true;
            part = part.slice(4).trim();
        } else if (part.startsWith("only ")) {
            part = part.slice(5).trim();
        }
        let result = true;
        for (const feature of part.split(/\s+and\s+/)) {
            const match = feature.trim().match(/^\(([^:)]+)(?::\s*([^)]+))?\)$/);
            if (!match) {
                result = false;
                break;
            }
            result = evaluateMediaFeature(match[1].trim(), (match[2] || "").trim());
            if (!result) {
                break;
            }
        }
        return negate ? !result : result;
    };

    const evaluateMediaQuery = (query) =>
        String(query)
            .split(",")
            .some(evaluateMediaQueryPart);

    if (typeof globalThis.matchMedia !== "function") {
        globalThis.matchMedia = function matchMedia(query) {
            const media = String(query);
            const listeners = new Set();
            const list = {
                media,
                onchange: null,
                get matches() {
                    return evaluateMediaQuery(media);
                },
                addListener(listener) {
                    if (typeof listener === "function") {
                        listeners.add(listener);
                    }
                },
                removeListener(listener) {
                    listeners.delete(listener);
                },
                addEventListener(type, listener) {
                    if (type === "change" && typeof listener === "function") {
                        listeners.add(listener);
                    }
                },
                removeEventListener(type, listener) {
                    if (type === "change") {
                        listeners.delete(listener);
                    }
                },
                dispatchEvent(event) {
                    for (const listener of listeners) {
                        try {
                            listener.call(list, event);
                        } catch (_) {}
                    }
                    if (typeof list.onchange === "function") {
                        try {
                            list.onchange.call(list, event);
                        } catch (_) {}
                    }
                    return true;
                },
            };
            return list;
        };
    }

    if (typeof globalThis.screen === "undefined") {
        const orientation = {
            get type() {
                return window.innerHeight >= window.innerWidth
                    ? "portrait-primary"
                    : "landscape-primary";
            },
            get angle() {
                return window.innerHeight >= window.innerWidth ? 0 : 90;
            },
            addEventListener() {},
            removeEventListener() {},
            dispatchEvent() {
                return true;
            },
        };
        const screen = {};
        Object.defineProperties(screen, {
            width: { get: () => Math.round(Number(window.innerWidth) || 0), enumerable: true },
            height: { get: () => Math.round(Number(window.innerHeight) || 0), enumerable: true },
            availWidth: { get: () => Math.round(Number(window.innerWidth) || 0), enumerable: true },
            availHeight: { get: () => Math.round(Number(window.innerHeight) || 0), enumerable: true },
            colorDepth: { value: 24, enumerable: true },
            pixelDepth: { value: 24, enumerable: true },
            orientation: { value: orientation, enumerable: true },
            addEventListener: { value() {} },
            removeEventListener: { value() {} },
        });
        globalThis.screen = screen;
    }

    if (typeof globalThis.performance === "undefined") {
        const timeOrigin = __blitz_performance_time_origin();
        globalThis.performance = {
            now: __blitz_performance_now,
            timeOrigin,
            mark() {},
            measure() {},
            clearMarks() {},
            clearMeasures() {},
            getEntries() {
                return [];
            },
            getEntriesByName() {
                return [];
            },
            getEntriesByType() {
                return [];
            },
            toJSON() {
                return { timeOrigin };
            },
            navigation: { type: 0, redirectCount: 0 },
            timing: {},
        };
    }

    if (typeof globalThis.requestIdleCallback !== "function") {
        const idleCallbacks = new Map();
        let nextIdleId = 1;
        globalThis.requestIdleCallback = function requestIdleCallback(callback, options) {
            const id = nextIdleId++;
            const requestedTimeout = Number(options && options.timeout);
            const timeout = Number.isFinite(requestedTimeout) && requestedTimeout > 0
                ? requestedTimeout
                : 0;
            const handle = setTimeout(
                function () {
                    idleCallbacks.delete(id);
                    if (typeof callback === "function") {
                        callback({
                            didTimeout: timeout > 0,
                            timeRemaining() {
                                return 50;
                            },
                        });
                    }
                },
                timeout > 0 ? Math.min(timeout, 1) : 1
            );
            idleCallbacks.set(id, handle);
            return id;
        };
        globalThis.cancelIdleCallback = function cancelIdleCallback(id) {
            const handle = idleCallbacks.get(id);
            if (handle !== undefined) {
                clearTimeout(handle);
                idleCallbacks.delete(id);
            }
        };
    }

    if (typeof globalThis.name === "undefined") {
        globalThis.name = "";
    }

    if (typeof navigator !== "undefined" && typeof navigator.connection === "undefined") {
        navigator.connection = {
            effectiveType: "4g",
            rtt: 100,
            downlink: 10,
            saveData: false,
            addEventListener() {},
            removeEventListener() {},
        };
    }
