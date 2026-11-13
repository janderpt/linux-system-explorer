# Compatibilidade D-Bus do Flatpak

A variante alpha.4 **flatpak-previews-dbus** usa **zbus 5.19.0**, cuja autenticação no Flatpak é sequencial. Isso evita a combinação de autenticação, negociação de descritores e primeira chamada em uma única escrita que falha no `xdg-dbus-proxy` 0.1.5. A negociação de descritores Unix continua habilitada: OpenFile e Print precisam dela para enviar o arquivo ao portal.

Não é necessário substituir manualmente o proxy do sistema para executar um **novo build com esta correção**. Use os pacotes mantidos pela distribuição e as atualizações normais do sistema. Os Flatpaks alpha.4 anteriores usam zbus 6 e não recebem esta alteração automaticamente; seus binários, checksums e requisitos históricos permanecem válidos. Para atualizar, instale a [variante com correção D-Bus](releases/v0.1.0-alpha.4-flatpak-dbus.md) na mesma branch, sem desinstalar ou apagar dados.

A adaptação mantém os serviços e as assinaturas D-Bus existentes. Não amplia as permissões do Flatpak, não desabilita o filtro do proxy, não remove a passagem de descritores e não altera as associações de aplicativos do host. As integrações nativas FileManager1, KIO, preferências de aparência e UDisks2 também usam a biblioteca adaptada; as limitações dessas integrações no Flatpak continuam documentadas em [FLATPAK.md](FLATPAK.md).

Os componentes independentes `zbus_names`, `zvariant` e `zvariant_derive` desta linha receberam avisos informativos de manutenção após sua incorporação ao zbus 6. Não há vulnerabilidade conhecida reportada no lock atual; os avisos não são suprimidos. A [revisão de terceiros](THIRD_PARTY_REVIEW.md) registra essa limitação da alpha e a necessidade de rever a estratégia de manutenção antes da versão estável/loja.

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

Referências de implementação: [autenticação do zbus 5.19.0](https://docs.rs/zbus/5.19.0/src/zbus/connection/handshake/client.rs.html) e [upstream xdg-dbus-proxy, problema 21](https://github.com/flatpak/xdg-dbus-proxy/issues/21).
