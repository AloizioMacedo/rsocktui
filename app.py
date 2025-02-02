from fastapi import FastAPI, WebSocket

app = FastAPI()


@app.websocket("/")
async def root(ws: WebSocket):
    await ws.accept()
    while True:
        await ws.send_text("Hello!")
        print("Hello!")
        received = await ws.receive_text()
        print(received)
