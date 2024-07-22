

ps aux | grep target | grep -E "validator|relayer" | awk '{print $2}' | xargs kill
