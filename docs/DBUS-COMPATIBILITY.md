# Compatibilidade D-Bus do Flatpak

A variante alpha.4 **flatpak-previews-dbus6** usa **zbus 6.2.1**, com um backport local da autenticação sequencial exigida por `xdg-dbus-proxy` **0.1.5**. Não depende mais dos componentes aposentados `zbus_names`, `zvariant` e `zvariant_derive`, e os três avisos D-Bus desapareceram da auditoria sem serem suprimidos. O aviso independente de `ttf-parser` continua registrado.

O upstream zbus 6 removeu o suporte ao proxy antigo. Por isso o projeto preserva uma cópia completa do crate oficial em [vendor/zbus](../vendor/zbus/README.lse.md), com a licença MIT original, origem/commit/SHA-256 e um [patch reproduzível](../packaging/patches/zbus-flatpak-auth.patch). Apenas `src/connection/handshake/client.rs` é alterado. O verificador [check-vendored-zbus.py](../scripts/check-vendored-zbus.py) recusa mudanças adicionais e hashes divergentes; roda no CI e antes do inventário de empacotamento. O pacote também inclui o fonte modificado, a licença, a proveniência e o diff junto dos avisos de terceiros.

No Linux Flatpak, AUTH aguarda sua resposta, depois NEGOTIATE_UNIX_FD aguarda a resposta do servidor; BEGIN/Hello continuam pelo fluxo upstream. As mesmas rotinas upstream validam autenticação, GUID e capacidade de descritores. Conexões nativas mantêm o pipelining original. Não há execução de proxy substituto nem autenticação própria no explorador. A passagem de descritores permanece necessária para OpenFile e Print.

O Flatpak **alpha.4 flatpak-previews-dbus já publicado** usa zbus 5.19 e continua com os avisos informativos registrados no seu relatório original. Seus binários, fontes, checksums e requisitos não foram substituídos. Para obter zbus 6.2.1, instale a [variante flatpak-previews-dbus6](releases/v0.1.0-alpha.4-flatpak-dbus6.md) na mesma branch, preservando as preferências.

A alteração não amplia permissões, não desativa a filtragem D-Bus, não reduz a validação do servidor e não modifica as associações do host. FileManager1, KIO, preferências de aparência e UDisks2 usam a API mantida. As limitações dessas integrações no Flatpak continuam em [FLATPAK.md](FLATPAK.md).

Para atualizar o zbus vendorizado, verifique o checksum do novo crate oficial, compare as mudanças upstream, porte ou remova o patch e atualize os hashes somente após revisão. Repita os testes dos proxies, portais, pares hostis e Flatpak instalado. Remova o override local quando o upstream oferecer compatibilidade equivalente; um patch local exige acompanhamento das atualizações upstream.

## Verificar o fonte incorporado

```sh
python3 scripts/check-vendored-zbus.py
cargo tree --locked -i zbus
cargo test --locked sequential_auth_rejects_invalid_peers_and_honors_fd_refusal
```

O teste de pares SASL simulados verifica rejeição de autenticação, GUID inválido/diferente e recusa de descritores. Depois da recusa, nenhum descritor é enviado. Ele usa sockets temporários e não acessa os serviços reais do desktop.

## Regressão com proxy real e barramento privado

Compile o executável de testes e obtenha seu caminho a partir do Cargo:

```sh
cargo test --locked --no-run --message-format=json > /tmp/lse-test-artifacts.jsonl
python3 - <<'PY'
import json, subprocess
from pathlib import Path
artifacts = [json.loads(line) for line in Path('/tmp/lse-test-artifacts.jsonl').read_text().splitlines()]
binary, = [a['executable'] for a in artifacts if a.get('reason') == 'compiler-artifact' and a.get('profile', {}).get('test') and a.get('executable')]
subprocess.run(['python3', 'scripts/test-dbus-proxy.py', binary], check=True)
PY
```

Requer `dbus-run-session`, Python 3 e `xdg-dbus-proxy`. O argumento opcional `--proxy /caminho/xdg-dbus-proxy` permite comparar versões sem instalar ou substituir um componente do host. O executável Rust precisa ser compatível com as bibliotecas do ambiente em que o teste roda. O CI executa essa regressão com o proxy da imagem Ubuntu 24.04.

O script cria seu próprio barramento e socket filtrado. Os portais simulados conectam diretamente ao barramento privado; apenas o cliente passa pelo proxy, com a mesma identidade Flatpak e o namespace privado de janelas. Não há chamadas ao portal real nem a impressoras. Confere:

- OpenFile/Abrir com: conteúdo recebido por descritor, nome com bytes especiais, sucesso, cancelamento e erro; inscrição de sinais antes da resposta.
- Print: descritor PDF somente para leitura, confirmação por trabalho, token da mesma conexão, falhas de preparação/envio e cancelamento com Close; nenhuma impressão real.
- Janelas: registro simultâneo de dois endpoints, Reveal com pasta e seleção preservadas, token de ativação e reutilização; negação de nomes não concedidos e de impersonação do portal.

Esses testes se somam aos testes nativos dos serviços FileManager1, KIO e UDisks2. Eles não certificam a apresentação dos diálogos reais de cada desktop.

## Instalação real do Flatpak em ambiente descartável

Reconstrua o bundle e execute [smoke-flatpak.py](../scripts/smoke-flatpak.py), com `--test-binary` e `--fixtures`, conforme [FLATPAK.md](FLATPAK.md). O teste inclui registro/reutilização de janelas através do proxy real do sandbox, instalação nova, atualização no mesmo ref, preferências preservadas, abertura gráfica em Xvfb e os conversores incluídos.

Para comparar versões já disponíveis no ambiente de teste, execute o mesmo teste em instalações descartáveis sequenciais, escolhendo o proxy pelo ambiente `FLATPAK_DBUSPROXY=/caminho/xdg-dbus-proxy`. Não altere essa variável na sessão normal do usuário para contornar o sandbox. O Dockerfile local usa `/usr/bin/xdg-dbus-proxy` da distribuição como padrão.

Referências de implementação: [migração upstream para zbus 6](https://z-galaxy.github.io/zbus/upgrading-to-6.html) e [upstream xdg-dbus-proxy, problema 21](https://github.com/flatpak/xdg-dbus-proxy/issues/21).
