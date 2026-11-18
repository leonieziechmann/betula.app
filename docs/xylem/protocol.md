# Xylem's protocol

What a browser and Xylem say to each other, what Xylem says to a push service, and what reaches
the browser. Version 1 (`/push/v1/`). Running Xylem: [`xylem.md`](xylem.md).

Xylem tells everybody who listens to a topic the same thing. It is not Betula Sync and knows
nothing of it: a subscription belongs to a browser, not to a profile.

## 1. Names

| | |
|---|---|
| subscription | a browser's push subscription (the Push API's `PushSubscription`): an `endpoint` at the browser's push service and two keys, `p256dh` (a P-256 public key, 65 bytes uncompressed) and `auth` (16 bytes). One per browser and site; an installed web app on an iPhone has its own |
| VAPID key | Xylem's key pair (RFC 8292). A browser subscribes with its public half; a push service takes a message for that subscription only with a token the private half signed |
| topic | what a subscription listens to: lower-case letters, digits and hyphens, a letter first, 32 at most. Xylem keeps no list of them: a topic is whatever Folia offers and an announcement names |
| language | the language a subscription's texts are in: `de`, `en`, or with a region `en-gb` (lower case as kept) |
| announcement | a title, a body and a link by language, for one topic, told from a time for a time |

The topics of today:

| topic | says |
|---|---|
| `exams` | the exam period and what leads to it (registration opens, it begins) |
| `semester` | the semester's turn: the next semester's dates are there, it begins, re-registration |
| `breaks` | when the lectures pause and resume (Christmas, the weeks without lectures) |

What the site announces by itself under them: section 7.

## 2. What Xylem holds, and what it never does

- Of a browser: its endpoint, its two keys, its language, its topics, and when it came. Not the
  address it came from, no profile, no account, no cookie, not when it was last there.
- It writes to an endpoint only where it is a known push service's (xylem.md, „The push
  services"), and only what was announced: there is no message to one browser alone.
- Announcements are made on Xylem's own machine (section 6) and read from the site's own feed
  (section 7). Nothing a browser sends makes Xylem send anything.
- Who knows an endpoint may replace or delete its subscription: an endpoint is a secret between
  the browser, its push service and Xylem, and is never logged or shown.
- A push service sees that Betula's key wrote to a subscription, when and how much; never what
  (section 3).

## 3. Browser to Xylem

JSON over HTTPS, on the site's own host. Errors are `{"error": "<code>"}`.
`Cache-Control: no-store` on everything. A request with an `Origin` that is not the site's is
refused (403 `origin`).

### `GET /push/v1/key`

```json
{"key": "BBZjaIEO8udcHcLjaeV7gqtdafvw6YJhYgifJvO4YOt6tZcZKfWcJuEA4aSe9o607xixfBnBaQCVP2zvpkZIdB0"}
```

The public half of the VAPID key, base64url: the `applicationServerKey` of
`pushManager.subscribe`.

### `PUT /push/v1/subscription`

The subscription as the browser writes it (`subscription.toJSON()`), with a language and topics:

```json
{
  "endpoint": "https://fcm.googleapis.com/fcm/send/dQw4w9WgXcQ:APA91b…",
  "expirationTime": null,
  "keys": {"p256dh": "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4", "auth": "BTBZMqHH6r4Tts7J_aSIgg"},
  "lang": "de",
  "topics": ["exams", "semester"]
}
```

204. Put again, it replaces what Xylem held for the endpoint (keys, language, topics): this is
how a browser changes its topics, and how it says at every start that it is still there. With
`"topics": []` the subscription is forgotten. `lang` left out is the default language. Members
Xylem does not know are ignored.

| | |
|---|---|
| 400 `invalid` | no JSON, or keys that are no base64url |
| 400 `endpoint` | no endpoint of a push service Xylem writes to (the browser cannot be told through Xylem; Folia says so) |
| 400 `keys` | `p256dh` is no point of P-256 in 65 bytes, or `auth` is not 16 bytes |
| 400 `lang`, `topics` | a language or a topic that is none, or more than 16 topics |
| 413 `too_big` | a body above 32 KiB |
| 429 `slow_down` | too many from this address (`Retry-After`): 1200 writes or 120 new subscriptions an hour |
| 507 `full` | Xylem holds as many subscriptions as it may |

### `DELETE /push/v1/subscription`

```json
{"endpoint": "https://fcm.googleapis.com/fcm/send/dQw4w9WgXcQ:APA91b…"}
```

204, whether Xylem held one or not. 400 `endpoint` without one; 429 `slow_down`.

## 4. Xylem to the browser

For every subscription of an announcement's topic, Xylem sends its push service one request
(RFC 8030):

```
POST <endpoint>
TTL: 86400
Urgency: normal
Content-Encoding: aes128gcm
Content-Type: application/octet-stream
Authorization: vapid t=<token>, k=<the VAPID key's public half>

<the message, sealed for this subscription>
```

- **Sealed** per RFC 8291: ECDH over P-256 with a key pair made for this one message, HKDF-SHA256
  with the subscription's `auth`, AES-128-GCM, one record. Only the browser opens it.
- **The token** (RFC 8292): a JWT signed ES256 with the VAPID key, `aud` the endpoint's origin,
  `exp` twelve hours ahead, `sub` Xylem's contact address.
- **`TTL`**: the seconds left of the time the announcement is told. A device that is off gets
  the message when it comes back within them, and never after.

The message is a declarative push message (the Push API, „Declarative push message"):

```json
{
  "web_push": 8030,
  "notification": {
    "title": "Die Prüfungsphase beginnt",
    "body": "Ab heute laufen die Prüfungen. Viel Erfolg!",
    "navigate": "https://betula.app/studyplan",
    "lang": "de",
    "tag": "exams-2027-02",
    "timestamp": 1801461600000,
    "data": {"topic": "exams"}
  }
}
```

| member | |
|---|---|
| `title` | always there |
| `body` | left out where the announcement has none |
| `navigate` | what a tap opens: the site's origin and the announcement's link for this language, always an address of the site |
| `lang` | the language of this text: the subscription's, or the nearest the announcement has (`en` for `en-gb`, else the default) |
| `tag` | the announcement's key, else `xylem-<id>`: one announcement is one notification, and one that arrives twice (section 8) takes its own place where the browser knows tags |
| `timestamp` | when the announcement was told (milliseconds), not when it arrived |
| `data.topic` | the topic it was told for |

No `mutable`: the message says everything, and nothing has to run to show it. At most 3993
bytes, what one push message carries.

## 5. What Folia's part does

Not built yet (xylem.md, „Not yet"); this is what the contract asks of it.

**Subscribing**, from a tap (every browser asks for the permission only then; on an iPhone only
in the web app on the home screen):

1. `GET /push/v1/key`.
2. `registration.pushManager.subscribe({userVisibleOnly: true, applicationServerKey: key})`.
3. `PUT /push/v1/subscription` with `{...subscription.toJSON(), lang, topics}`.

**At every start**, where the permission is granted and topics are chosen: read the key again,
and if the subscription was made with another one (`subscription.options.applicationServerKey`),
`unsubscribe()` and subscribe anew; then `PUT` the subscription again. That one request is how a
Xylem that lost its database (and with it its key) has every browser again without anybody doing
anything, and how a change of language reaches it. A `pushsubscriptionchange` in the service
worker does the same for the new subscription and deletes the old endpoint.

**Changing the topics**: `PUT` again. **Turning it off**: `DELETE`, then
`subscription.unsubscribe()`.

**Showing a message.** Safari (on an iPhone or iPad from 18.4, on a Mac from 18.5) shows a
declarative message by itself, without the service worker. Every other browser, and an older Safari, hands
it to the service worker, which shows what it says, with the site's own icons:

```js
self.addEventListener("push", (event) => {
  const message = event.data?.json();
  if (message?.web_push !== 8030) return;
  const n = message.notification;
  event.waitUntil(self.registration.showNotification(n.title, {
    body: n.body, lang: n.lang, tag: n.tag, timestamp: n.timestamp,
    data: {...n.data, navigate: n.navigate},
    icon: "/assets/icon-192.png", badge: "/assets/icon-monochrome-512.png",
  }));
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  event.waitUntil(self.clients.openWindow(event.notification.data.navigate));
});
```

A `push` must always end in a notification: a browser that is woken and shows nothing takes the
subscription away (Safari) or shows a notice of its own (Chrome).

**The privacy notice** names what section 2 lists, and that the message goes through the
browser's push service (Google, Mozilla, Apple or Microsoft, whichever the browser uses).

## 6. The commands' side

For `xylem announce`, `announcements`, `cancel`, `feed` and `status` (xylem.md, „Commands"): plain HTTP
on the same listener, answered only for a loopback peer without `X-Real-Ip`, `X-Forwarded-For` or
`Origin` (so not through Traefik, and not for a page open on the machine); anybody else gets 404.
It is no part of the contract with browsers and changes with Xylem's commands.

| | |
|---|---|
| `GET /status` | version, site, default language, the public key, subscriptions in all, by topic and by language, announcements not finished by state, attempts that wait, when the next one is due, hosts refused |
| `POST /announcements` | `{"topic", "texts": {"de": {"title", "body", "link"}}, "at", "for", "urgency", "link", "key", "dry"}` → 201 `{"announcement", "made": true, "reach": {"de": 412}}`; 200 with `"made": false` for a key that was taken (the one that has it) and for a dry run; 400 `{"error": "invalid", "detail": "…"}` |
| `GET /announcements?limit=` | `{"lang", "announcements": […]}`, the newest first, each with who made it (`source`: `command` or `feed`) |
| `DELETE /announcements/<id>` | 200; 404 `not_found`; 409 `finished` |
| `POST /feed` | reads the feed now: `{"entries", "baseline", "unchanged", "changes": [{"key", "what", "at", "detail"}]}`; 409 `no_feed`, 502 `feed` |

## 7. The feed

What Betula announces by itself: an address of the site that Xylem reads (`XYLEM_FEED`, every
ten minutes, with `If-None-Match`). Folia's is `GET /api/announcements`; the contract is between
Folia's server and Xylem, and no browser is part of it.

```json
{
  "announcements": [
    {
      "key": "exams-2026w-1",
      "topic": "exams",
      "at": "2027-02-08T07:00:00+01:00",
      "for": "12h",
      "link": "/",
      "texts": {
        "de": {"title": "Die Prüfungsphase beginnt", "body": "Bis 20.02. liegen die meisten Prüfungen des WiSe 2026/27. Viel Erfolg!"},
        "en": {"title": "The exam period begins", "body": "Most exams of Winter 2026/27 are held until 20 Feb. Good luck!", "link": "/en"}
      }
    }
  ]
}
```

An entry is what `POST /announcements` takes (section 6), with these differences:

| | |
|---|---|
| `key` | needed: it is what the entry is known by from one read to the next. The same key with another time or text is the same announcement, changed; a key no longer named is an announcement withdrawn |
| `at` | left out for what is so from now on and has no hour of its own (the next semester's dates being there): Xylem tells it when it first reads it, between 08:00 and 20:00, and never on a feed's first read |
| the document | 200 and a member `announcements`, an array (empty: nothing to announce). Anything else (another status, no such member) says nothing, and Xylem changes nothing: a site without a snapshot answers 503, not an empty feed |
| an `ETag` | Xylem sends it back as `If-None-Match`; 304 is „what it was" |

The feed names what is to be told from a few days back on and everything ahead; what Xylem did
with an entry it keeps itself, so an entry that is told need not be named any longer. What Xylem
does with the feed, and its bounds (four within a day, the hours of the day, a person's word
above the feed's): xylem.md, „The feed".

## 8. What may happen

- **A message twice.** Xylem records a step of 100 subscriptions after it wrote to them; killed
  in between, it writes that step again. With the same `tag`, the second takes the first one's
  place silently (Chrome, Firefox, Edge); Safari, which knows no tags, shows both.
- **A message late or never.** A push service delivers when the device is reachable. Within
  `TTL` it arrives late (`timestamp` says from when it is); after it, never. A subscription whose
  push service was unreachable through every attempt of an announcement misses it.
- **A subscription gone.** A browser whose data was cleared, whose permission was taken back or
  whose web app was removed: the push service answers 404 or 410 at the next announcement, and
  Xylem forgets it.
- **Xylem's database lost.** Every subscription and the key with it. Browsers come back by
  themselves at their next start (section 5); what was announced meanwhile, they missed.

## 9. Versions

`/push/v1/` is this contract. What breaks a browser of an older build is a MAJOR change of Xylem
(CLAUDE.md, „Versions"): a member of section 3 or 4 that goes away or means something else,
another shape of the message. New members a browser ignores, new topics and new hosts of push
services are not. The feed (section 7) is read by the Xylem that runs beside its Folia: a member
of it that goes away breaks that pair, and is changed in both at once.
