import csv
import datetime
import random
import os

def generate_csv(ticker, days=10, filename=None):
    if not filename:
        filename = f"{ticker}.csv"
        
    start_date = datetime.datetime.now() - datetime.timedelta(days=days)
    base_price = 220.0
    
    with open(filename, 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(['ts', 'o', 'h', 'l', 'c', 'v'])
        
        current_time = start_date
        price = base_price
        
        while current_time < datetime.datetime.now():
            # Skip weekends
            if current_time.weekday() >= 5:
                current_time += datetime.timedelta(days=1)
                current_time = current_time.replace(hour=9, minute=30)
                continue
                
            # Market hours (approximate for generation, UTC/Local handled by tool)
            # Generating in UTC. Assume standard time (offset -5).
            # 09:30 ET is 14:30 UTC. 16:00 ET is 21:00 UTC.
            
            # Simple day loop
            day_start = current_time.replace(hour=14, minute=30, second=0, microsecond=0)
            day_end = current_time.replace(hour=21, minute=0, second=0, microsecond=0)
            
            if current_time < day_start:
                current_time = day_start
                
            if current_time >= day_end:
                 current_time += datetime.timedelta(days=1)
                 current_time = current_time.replace(hour=14, minute=30)
                 continue
                 
            # Generate minute bar
            o = price
            change = random.uniform(-0.5, 0.5)
            c = o + change
            h = max(o, c) + random.uniform(0, 0.2)
            l = min(o, c) - random.uniform(0, 0.2)
            v = random.randint(1000, 50000)
            
            # Format: 2025-12-20T15:31:00Z
            ts_str = current_time.strftime('%Y-%m-%dT%H:%M:%SZ')
            writer.writerow([ts_str, f"{o:.2f}", f"{h:.2f}", f"{l:.2f}", f"{c:.2f}", v])
            
            price = c
            current_time += datetime.timedelta(minutes=1)

    print(f"Generated {filename}")

if __name__ == "__main__":
    generate_csv("AMZN")
