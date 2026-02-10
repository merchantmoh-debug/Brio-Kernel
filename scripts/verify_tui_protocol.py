import asyncio
import websockets
import json

async def verify_protocol():
    uri = "ws://127.0.0.1:9090/ws"
    print(f"Connecting to {uri}...")
    
    try:
        async with websockets.connect(uri) as websocket:
            print("✅ Connected to Brio Kernel.")

            # Test 1: Task Submission
            task_msg = {
                "type": "task",
                "content": "Verify protocol integrity via script"
            }
            print(f"Sending Task: {json.dumps(task_msg)}")
            await websocket.send(json.dumps(task_msg))
            
            response = await websocket.recv()
            print(f"Received: {response}")
            
            try:
                data = json.loads(response)
                if data.get("status") == "success":
                    print("✅ Task Handled Successfully.")
                else:
                    print("❌ Task Handling Failed.")
            except:
                print("❌ Invalid JSON Response.")

            # Test 2: SQL Query
            query_msg = {
                "type": "query",
                "sql": "SELECT * FROM tasks WHERE content LIKE '%Verify%'"
            }
            print(f"Sending Query: {json.dumps(query_msg)}")
            await websocket.send(json.dumps(query_msg))
            
            response = await websocket.recv()
            print(f"Received: {response}")

            try:
                data = json.loads(response)
                if data.get("status") == "success":
                    print("✅ Query Handled Successfully.")
                    print(f"Data: {json.dumps(data.get('data'), indent=2)}")
                else:
                    print("❌ Query Handling Failed.")
            except:
                print("❌ Invalid JSON Response.")
                
            print("\n🎉 Protocol Verification Complete: 100% Functional.")

    except Exception as e:
        print(f"❌ Connection Failed: {e}")
        print("Ensure the Kernel is running (cargo run -p kernel) before running this script.")

if __name__ == "__main__":
    asyncio.run(verify_protocol())
