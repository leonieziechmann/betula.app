"""Opens N idle server-sent-event streams and holds them for a while.

    python3 client.py 10000          # straight to the server (127.0.0.1:18777)
    python3 client.py 8000 18778     # through the Go proxy

The server and the proxy print their own resident memory every two seconds (stderr): the
difference between none and N open streams, divided by N, is what one stream costs there.
"""
import asyncio
import resource
import sys

N = int(sys.argv[1])
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 18777

soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
resource.setrlimit(resource.RLIMIT_NOFILE, (min(hard, N + 1000), hard))

conns = []


async def one():
    r, w = await asyncio.open_connection("127.0.0.1", PORT)
    w.write(b"GET /events HTTP/1.1\r\nHost: b\r\nAccept: text/event-stream\r\n\r\n")
    await w.drain()
    await r.readuntil(b"\r\n\r\n")
    conns.append((r, w))


async def main():
    for i in range(0, N, 500):
        await asyncio.gather(*(one() for _ in range(min(500, N - i))))
    print("opened", len(conns), flush=True)
    await asyncio.sleep(12)


asyncio.run(main())
