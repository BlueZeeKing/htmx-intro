const name_value = () => document.querySelector("input").value;

const signin = document.querySelector("#login");
const register = document.querySelector("#register");

const authenticate = async () => {
    if (name_value() == "") {
        return;
    }

    const [options, id] = await (await fetch("/start-login", {
        method: "POST",
        body: JSON.stringify({
            name: name_value()
        }),
        headers: {
            "Content-type": "application/json; charset=UTF-8"
        }
    })).json()

    options.publicKey.challenge = Base64.toUint8Array(options.publicKey.challenge);
    options.publicKey.allowCredentials.forEach((item) => item.id = Base64.toUint8Array(item.id));

    const credential = await navigator.credentials.get(options);

    let token = await (await fetch("/finish-login", {
        method: "POST",
        body: JSON.stringify([id, {
            id: credential.id,
            rawId: Base64.fromUint8Array(new Uint8Array(credential.rawId), true),
            response: {
                authenticatorData: Base64.fromUint8Array(new Uint8Array(credential.response.authenticatorData), true),
                clientDataJSON: Base64.fromUint8Array(new Uint8Array(credential.response.clientDataJSON), true),
                signature: Base64.fromUint8Array(new Uint8Array(credential.response.signature), true),
                userHandle: Base64.fromUint8Array(new Uint8Array(credential.response.userHandle), true)
            },
            type: credential.type,
        }]),
        headers: {
            "Content-type": "application/json; charset=UTF-8"
        }
    })).text();

    document.cookie = "SessionToken=" + token;

    window.location.href = "/";
}

signin.addEventListener("click", async () => {
    await authenticate()
})

register.addEventListener("click", async () => {
    if (name_value() == "") {
        return;
    }

    const [options, id] = await (await fetch("/start-register", {
        method: "POST",
        body: JSON.stringify({
            name: name_value()
        }),
        headers: {
            "Content-type": "application/json; charset=UTF-8"
        }
    })).json();

    options.publicKey.challenge = Base64.toUint8Array(options.publicKey.challenge);
    options.publicKey.user.id = Base64.toUint8Array(options.publicKey.user.id);

    let credential = await navigator.credentials.create(options);

    await fetch("/finish-register", {
        method: "POST",
        body: JSON.stringify([id, {
            id: credential.id,
            rawId: Base64.fromUint8Array(new Uint8Array(credential.rawId), true),
            response: {
                attestationObject: Base64.fromUint8Array(new Uint8Array(credential.response.attestationObject), true),
                clientDataJSON: Base64.fromUint8Array(new Uint8Array(credential.response.clientDataJSON), true),
            },
            type: credential.type
        }]),
        headers: {
            "Content-type": "application/json; charset=UTF-8"
        }
    });

    await authenticate();
})
