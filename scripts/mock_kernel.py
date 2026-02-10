import asyncio
import websockets
import json

async def handler(websocket):
    print("TUI Connected to Mock Kernel")
    try:
        # Send a welcome log
        welcome_msg = {
            "type": "message",
            "log": "Mock Kernel Initialized. Welcome to Brio TUI."
        }
        await websocket.send(json.dumps(welcome_msg))
        
        async for message in websocket:
            print(f"Received from TUI: {message}")
            try:
                data = json.loads(message)
                # response = {"status": "success", "data": None}
                
                if data.get("type") == "task":
                    print(f"   [Task] {data.get('content')}")
                    await websocket.send(json.dumps({
                        "type": "message",
                        "log": f"Task received: {data.get('content')}"
                    }))
                elif data.get("type") == "query":
                    print(f"   [Query] {data.get('sql')}")
                    await websocket.send(json.dumps({
                        "type": "message",
                        "log": f"Executing Query: {data.get('sql')}"
                    }))
                
                # Send generic success response
                # (The real kernel sends a specific response structure, simplistic here)
                
            except json.JSONDecodeError:
                print("Invalid JSON received")

    except websockets.exceptions.ConnectionClosed:
        print("TUI Disconnected")

async def main():
    print("Mock Kernel listening on ws://127.0.0.1:9090/ws")
    async with websockets.serve(handler, "127.0.0.1", 9090):
        await asyncio.Future()  # run forever

if __name__ == "__main__":
    asyncio.run(main())
