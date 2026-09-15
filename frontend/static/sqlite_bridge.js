/**
 * BTU Client-Side SQLite Engine Bridge (sql.js + IndexedDB Caching)
 */
(function () {
    let sqlEngine = null;
    let activeDb = null;
    const DB_NAME = "BTU_CACHE_V2";
    const STORE_NAME = "sqlite_snapshots";

    function openIndexedDB() {
        return new Promise((resolve, reject) => {
            const req = indexedDB.open(DB_NAME, 1);
            req.onupgradeneeded = (e) => {
                const db = e.target.result;
                if (!db.objectStoreNames.contains(STORE_NAME)) {
                    db.createObjectStore(STORE_NAME, { keyPath: "id" });
                }
            };
            req.onsuccess = (e) => resolve(e.target.result);
            req.onerror = (e) => reject(e.target.error);
        });
    }

    async function getCachedSnapshot(id) {
        try {
            const idb = await openIndexedDB();
            return new Promise((resolve) => {
                const tx = idb.transaction(STORE_NAME, "readonly");
                const store = tx.objectStore(STORE_NAME);
                const req = store.get(id);
                req.onsuccess = () => resolve(req.result);
                req.onerror = () => resolve(null);
            });
        } catch (e) {
            console.warn("IndexedDB read error:", e);
            return null;
        }
    }

    async function saveSnapshot(id, etag, buffer) {
        try {
            const idb = await openIndexedDB();
            return new Promise((resolve) => {
                const tx = idb.transaction(STORE_NAME, "readwrite");
                const store = tx.objectStore(STORE_NAME);
                const req = store.put({ id, etag, buffer, timestamp: Date.now() });
                req.onsuccess = () => resolve(true);
                req.onerror = (e) => reject(e.target.error);
            });
        } catch (e) {
            console.warn("IndexedDB write error:", e);
        }
    }

    window.initBtuDatabase = async function (dbUrl, statusUrl, onProgressCallback) {
        try {
            if (!sqlEngine) {
                if (typeof window.initSqlJs !== "function") {
                    throw new Error("sql-wasm.js not loaded.");
                }
                sqlEngine = await window.initSqlJs({
                    locateFile: file => `/static/${file}`
                });
            }

            let remoteEtag = null;
            let expectedTotalBytes = 0;
            try {
                const statusRes = await fetch(statusUrl || "/api/status");
                if (statusRes.ok) {
                    const statusData = await statusRes.json();
                    remoteEtag = (statusData.database && statusData.database.etag) || statusData.etag || null;
                    expectedTotalBytes = (statusData.database && statusData.database.size_bytes) || statusData.db_size_bytes || 0;
                }
            } catch (err) {
                console.warn("Could not check /api/status:", err);
            }

            const cached = await getCachedSnapshot("btu_modules.db");
            if (cached && cached.buffer && (remoteEtag == null || cached.etag === remoteEtag)) {
                console.log("[DB Bridge] Loading SQLite DB from IndexedDB cache (ETag: " + cached.etag + ")");
                activeDb = new sqlEngine.Database(new Uint8Array(cached.buffer));
                if (typeof onProgressCallback === "function") {
                    onProgressCallback(100, "Bereit aus lokalem Cache");
                }
                return JSON.stringify({ status: "ready", source: "cache", etag: cached.etag });
            }

            console.log("[DB Bridge] Downloading database from " + dbUrl + "...");
            if (typeof onProgressCallback === "function") {
                onProgressCallback(5, "Verbinde mit BTU API Server...");
            }

            const response = await fetch(dbUrl || "/api/db");
            if (!response.ok) {
                throw new Error("Fehler beim Herunterladen der Datenbank: HTTP " + response.status);
            }

            const responseEtag = response.headers.get("ETag") || remoteEtag || "v1";
            const contentLengthHeader = response.headers.get("Content-Length");
            const totalBytes = contentLengthHeader ? parseInt(contentLengthHeader, 10) : expectedTotalBytes;

            const reader = response.body.getReader();
            let receivedBytes = 0;
            const chunks = [];

            while (true) {
                const { done, value } = await reader.read();
                if (done) break;
                chunks.push(value);
                receivedBytes += value.length;

                if (totalBytes > 0 && typeof onProgressCallback === "function") {
                    const pct = Math.min(99, Math.round((receivedBytes / totalBytes) * 100));
                    const mb = (receivedBytes / (1024 * 1024)).toFixed(1);
                    const totalMb = (totalBytes / (1024 * 1024)).toFixed(1);
                    onProgressCallback(pct, `${mb} MB / ${totalMb} MB (${pct}%)`);
                }
            }

            const completeBuffer = new Uint8Array(receivedBytes);
            let position = 0;
            for (const chunk of chunks) {
                completeBuffer.set(chunk, position);
                position += chunk.length;
            }

            activeDb = new sqlEngine.Database(completeBuffer);
            saveSnapshot("btu_modules.db", responseEtag, completeBuffer.buffer).catch(console.warn);

            if (typeof onProgressCallback === "function") {
                onProgressCallback(100, "Datenbank geladen");
            }

            console.log("[DB Bridge] SQLite DB ready (" + receivedBytes + " Bytes)");
            return JSON.stringify({ status: "ready", source: "network", etag: responseEtag, bytes: receivedBytes });
        } catch (err) {
            console.error("[DB Bridge] Fehler:", err);
            throw err;
        }
    };

    window.dbQuery = function (sql, paramsJson) {
        if (!activeDb) {
            throw new Error("Datenbank noch nicht initialisiert");
        }
        let params = [];
        if (paramsJson) {
            try {
                params = JSON.parse(paramsJson);
            } catch (e) {
                params = [];
            }
        }

        const stmt = activeDb.prepare(sql);
        if (params && params.length > 0) {
            stmt.bind(params);
        }

        const rows = [];
        while (stmt.step()) {
            rows.push(stmt.getAsObject());
        }
        stmt.free();
        return JSON.stringify(rows);
    };

    window.dbQueryValue = function (sql, paramsJson) {
        if (!activeDb) return null;
        let params = [];
        if (paramsJson) {
            try {
                params = JSON.parse(paramsJson);
            } catch (e) {}
        }
        const stmt = activeDb.prepare(sql);
        if (params && params.length > 0) {
            stmt.bind(params);
        }
        let val = null;
        if (stmt.step()) {
            const row = stmt.get();
            if (row && row.length > 0) {
                val = row[0];
            }
        }
        stmt.free();
        return val != null ? String(val) : null;
    };
})();
