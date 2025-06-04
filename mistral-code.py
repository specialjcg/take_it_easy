#!/usr/bin/env python3
import os
import sys
import readline
import requests

API_KEY = os.getenv("MISTRAL_API_KEY")
if not API_KEY:
    print("❌ MISTRAL_API_KEY manquant. Ajoutez-le à votre fichier shell.")
    sys.exit(1)

MODEL = "mistral-small-2501"

def call_mistral(prompt, context=""):
    headers = {
        "Authorization": f"Bearer {API_KEY}",
        "Content-Type": "application/json"
    }
    messages = [
        {"role": "user", "content": f"{prompt}\n\n{context}"}
    ]
    data = {"model": MODEL, "messages": messages}

    try:
        response = requests.post(
            "https://api.mistral.ai/v1/chat/completions",
            headers=headers,
            json=data,
            verify=False  # <-- Désactivation SSL pour environnements filtrés
        )
        return response.json()["choices"][0]["message"]["content"].strip()
    except Exception as e:
        return f"❌ Erreur API : {e}\n{response.text if 'response' in locals() else ''}"

def read_code_files():
    code = ""
    extensions = ('.py', '.rs', '.js', '.ts', '.java', '.cpp', '.c', '.go')
    for root, dirs, files in os.walk("."):
        for file in files:
            if file.endswith(extensions):
                filepath = os.path.join(root, file)
                try:
                    with open(filepath, encoding="utf-8", errors="ignore") as f:
                        rel_path = os.path.relpath(filepath)
                        code += f"\n\n# Fichier : {rel_path}\n" + f.read()
                except Exception as e:
                    code += f"\n\n# Fichier : {filepath} (non lisible : {e})\n"
    return code or "[Aucun fichier code détecté dans ce projet]"


def main():
    print("╭──────────────────────────────────────────────────────────────╮")
    print("│ ✻ Welcome to Mistral Code!                                   │")
    print("│                                                              │")
    print("│   /help for help, /status for current setup                  │")
    print(f"│   cwd: {os.getcwd()}".ljust(62) + "│")
    print("╰──────────────────────────────────────────────────────────────╯")

    while True:
        try:
            user_input = input("╭─ > ").strip()
        except (EOFError, KeyboardInterrupt):
            print("\n👋 Au revoir.")
            break

        if user_input == "/exit":
            print("👋 Sortie.")
            break
        elif user_input == "/help":
            print("🔹 Tape une instruction (ex: analyser le code)")
            print("🔹 /status – voir modèle et contexte")
            print("🔹 /exit – quitter")
        elif user_input == "/status":
            print(f"📄 Modèle : {MODEL}")
            print(f"📂 Dossier : {os.getcwd()}")
            print("📁 Fichiers visibles :", ", ".join(os.listdir()))
        elif user_input:
            context = read_code_files()
            print("⏳ Envoi à Mistral...")
            response = call_mistral(user_input, context)
            print("\n" + response + "\n")

if __name__ == "__main__":
    main()
