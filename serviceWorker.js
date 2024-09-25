var CACHE_NAME = 'MINION-CACHE';
var urlsToCache = [
    '/',
];


/* Start the service worker and cache all of the app's content */
// Install event: cache the URLs
self.addEventListener('install', function(event) {
    self.skipWaiting();
    event.waitUntil(
        caches.open(CACHE_NAME).then((cache) => {
            return cache.addAll([
                urlsToCache
            ]);
        })
    );
});

/* Serve cached content when offline */
self.addEventListener('fetch', (event) => {
    event.respondWith(
        caches.match(event.request).then((cachedResponse) => {
            if (cachedResponse) {
                // Check if the cached file's hash matches the requested one
                // You may need to implement your own logic to handle hash checks
                return cachedResponse; // Return the cached response if available
            }

            // If not in cache, perform a network fetch
            return fetch(event.request).then((networkResponse) => {
                // Optionally cache the new response for future use
                return caches.open('wasm-cache').then((cache) => {
                    cache.put(event.request, networkResponse.clone());
                    return networkResponse;
                });
            });
        })
    );
});
