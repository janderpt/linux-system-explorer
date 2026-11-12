# Flatpak experimental e preparação para o Flathub

Este pacote é um **protótipo local**, separado da alpha.2 nativa e ainda sem publicação no Flathub. A branch é `experimental`, com o mesmo ID `io.github.janderpt.linux-system-explorer`. A aplicação usa Freedesktop Platform **26.08**. A instalação nativa continua sendo a referência com todos os conversores e integrações do sistema.

## Instalar e remover o protótipo

Use Flatpak atualizado e `xdg-dbus-proxy` **0.1.9 ou posterior**, mantidos pela sua distribuição. A biblioteca zbus 6 precisa do protocolo do proxy 0.1.6+, e o teste deste pacote usa 0.1.9 com correções atuais. O proxy 0.1.5 da imagem Ubuntu 24.04 original não funciona para as chamadas D-Bus do aplicativo; instalar um runtime novo não atualiza esse componente do host. Referências: [zbus 6](https://z-galaxy.github.io/zbus/upgrading-to-6.html) e [proxy 0.1.9](https://github.com/flatpak/xdg-dbus-proxy/releases/tag/0.1.9).

Com esses componentes instalados pela sua distribuição, adicione o repositório de runtimes e instale o arquivo recebido:

```sh
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user ./linux-system-explorer-0.1.0-alpha.2-flatpak-experimental-x86_64.flatpak
flatpak run --branch=experimental io.github.janderpt.linux-system-explorer
```

O repositório fornece o runtime; o aplicativo acima vem do arquivo local, **não da loja**. O bundle não inclui o runtime e precisa de rede na primeira instalação. Os checksums acompanham o arquivo em `SHA256SUMS`; `FLATPAK-BUILD-INFO.json` identifica fonte, binário, runtime, permissões e recursos detectados. Este protótipo não tem assinatura GPG nem atualização automática por um repositório próprio.

Para atualizar, feche as janelas Flatpak e repita a instalação com o novo arquivo. Para remover somente este protótipo:

```sh
flatpak uninstall --user io.github.janderpt.linux-system-explorer//experimental
```

As preferências ficam em `~/.var/app/io.github.janderpt.linux-system-explorer/`, separadas da instalação nativa; não copie nem sobrescreva as configurações nativas automaticamente. Nenhum desses comandos altera o explorador padrão ou registra FileManager1. Execute explicitamente o comando Flatpak enquanto coexistirem as duas instalações.

## Recursos e diferenças do sandbox

| Área | Protótipo | Antes de distribuição na loja |
| --- | --- | --- |
| Pastas, abas, dois painéis, layouts e operações | Mesma implementação Rust; acesso aos arquivos autorizado pelo Flatpak | Exercitar arquivos descartáveis em KDE/GNOME, Wayland/X11 e mídias externas |
| Imagens, SVG e texto | Decodificação incorporada, incluindo miniaturas de imagens | Conferir limites e imagens grandes em máquinas limpas |
| Áudio e vídeo | GStreamer do runtime e extensão de codecs; sem dependência do player do host | Validar codecs e áudio na sessão real |
| Compactados | Python e libarchive do runtime | Conferir ZIP, TAR, 7z e RAR |
| PDF, Office e HTML/Markdown formatado | Conversores ainda não incluídos; fonte HTML/Markdown continua disponível e as miniaturas mantêm o ícone | Incluir Poppler, LibreOffice e WeasyPrint a partir de fontes e revisar licenças/tamanho; instalar esses componentes apenas no host não resolve |
| Abrir e Abrir com | OpenFile do portal, passando descritor do arquivo; o seletor e as associações ficam com o desktop | Conferir o backend de portal KDE/GNOME e cancelamento |
| Associação padrão de tipos de arquivo | A escolha fica com o portal; não há gravação direta no registro MIME do host | Avaliar o fluxo oferecido por cada backend |
| Terminal integrado | Shell e programas disponíveis **dentro do Flatpak**; identificado como Terminal (Flatpak) | Não apresentar como terminal administrativo/do host |
| Compartilhamentos Samba locais e novas conexões remotas | Gerenciamento local oculto; novas conexões informam que a integração ainda não está disponível | Projetar integração apropriada; não adicionar execução irrestrita no host |
| Pastas de rede já montadas | Podem ser acessadas pelos caminhos expostos, incluindo GVfs | Validar permissões e autenticação na sessão real |
| Lixeira | Dados XDG próprios do sandbox; não pressupõe a mesma lista da Lixeira nativa | Validar envio, restauração, purga e volumes; definir a integração com a Lixeira do desktop |
| Mostrar na pasta e reutilização pelo sistema | O protótipo não assume o papel de FileManager1 | Projetar ativação D-Bus/exportação de serviços e testar sem conflitar com o gerenciador atual |

O protótipo pede leitura/escrita no filesystem `host` porque é um gerenciador de arquivos, incluindo pastas fora de Home; caminhos reservados pelo Flatpak continuam pertencendo ao runtime. Também pede Wayland/X11, OpenGL, áudio, IPC, GVfs e apenas o namespace privado de janelas do aplicativo no D-Bus. **Não** pede rede, barramento de sessão/sistema irrestrito, `org.freedesktop.Flatpak` ou execução de comandos no host. A amplitude do filesystem ainda precisa de justificativa e revisão humana para a loja. Veja as [permissões do Flatpak](https://docs.flatpak.org/en/latest/sandbox-permissions.html).

## Reproduzir o pacote local

Requer Python 3.11+, Rust, ferramentas de validação de metadados, Flatpak e o proxy D-Bus atualizado descrito acima. O script reutiliza o staging nativo com licenças/fontes/fontes MPL e verifica o executável sob o runtime antes de exportar. Não apresenta esse reempacotamento como uma compilação de fontes no SDK.

```sh
cargo build --release --locked
python3 scripts/flatpak.py stage
flatpak install --user flathub org.freedesktop.Platform//26.08
python3 scripts/flatpak.py build
```

Saída: `dist/flatpak-experimental/`. Os destinos devem estar vazios; não reutilize staging de outra revisão. `--binary`, `--destdir`, `--stage`, `--work-dir` e `--output` permitem separar ambientes de compilação/staging/exportação. O script não instala o aplicativo, muda padrões, cria tags ou publica arquivos. A ferramenta `flatpak-builder` não é necessária para esse caminho por comandos `flatpak build-*`.

O registro identifica alterações não commitadas. Para distribuir builds de teste rastreáveis, faça o staging de uma revisão limpa. O teste de ABI recusa bibliotecas não encontradas ou uma versão diferente do executável; isto não substitui testes funcionais dos codecs/portais.

## Ferramentas em container (opcional)

O [Dockerfile de ferramentas](../packaging/Dockerfile.flatpak-tools) permite exportar sem instalar Flatpak no host. Faça primeiro o staging nativo acima. O container é descartável e recebe somente a pasta do projeto como leitura; `--privileged` é necessário para o bubblewrap aninhado, apenas nesse ambiente de build/teste.

```sh
docker build -t lse-flatpak-tools -f packaging/Dockerfile.flatpak-tools .
mkdir -p build/flatpak-cache build/flatpak-work dist/flatpak-experimental
docker run --rm --privileged \
  -v "$PWD:/source:ro" \
  -v "$PWD/build/flatpak-cache:/root/.local/share/flatpak" \
  -v "$PWD/build/flatpak-work:/work" \
  -v "$PWD/dist/flatpak-experimental:/output" \
  lse-flatpak-tools sh -c '
    flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo &&
    flatpak install --user -y --noninteractive flathub org.freedesktop.Platform//26.08 &&
    python3 /source/scripts/flatpak.py build --stage /source/build/flatpak-stage/app --work-dir /work --output /output
  '
```

A imagem inclui o loader SVG exigido para exportar o ícone. Estes comandos exportam o bundle, sem instalar o aplicativo no host. Os testes gráficos em container usam D-Bus e Xvfb próprios; não substituem testes na sua sessão KDE/GNOME.

## Exemplo didático de manifesto — somente local

[flatpak-local.json](examples/flatpak-local.json) foi gerado com assistência de IA **para aprender e testar localmente**. Não é material de submissão ao Flathub e não deve ser usado como base do manifesto enviado à loja. Ele apenas copia um staging já compilado, por isso usa a Platform também como ambiente de cópia, sem toolchain. Não é um exemplo de compilação Cargo no SDK.

Depois do staging padrão acima, você pode testar esse caminho alternativo:

```sh
flatpak-builder --user --force-clean --repo=build/flatpak-example-repo build/flatpak-example docs/examples/flatpak-local.json
```

| Campo | Significado neste exemplo |
| --- | --- |
| `app-id` | Identidade constante; coincide com o desktop, o ícone e o AppStream |
| `runtime` e `runtime-version` | Bibliotecas fornecidas ao executar; branch da plataforma |
| `sdk` | Ambiente de build; aqui só copia arquivos já compilados; uma compilação Rust real usa um SDK com toolchain |
| `command` | Executável instalado em `/app/bin` |
| `branch` | Canal local `experimental`, separado de um eventual `stable` |
| `finish-args` | Acessos concedidos ao executar o aplicativo |
| `modules` | Etapas que colocam os arquivos em `/app` |
| `sources` | De onde vêm os arquivos; aqui um diretório local de staging |

Para aprender o formato a partir de material oficial, consulte o [primeiro build](https://docs.flatpak.org/en/latest/first-build.html), os [manifestos](https://docs.flatpak.org/en/latest/manifests.html) e o [gerador Cargo oficial](https://github.com/flatpak/flatpak-builder-tools/tree/master/cargo). Não envie este exemplo de binário local como se fosse um build offline de fontes.

## Caminho para a loja

O projeto ainda está em alpha. As [exigências do Flathub](https://docs.flathub.org/docs/for-app-authors/requirements) incluem software estável, histórico significativo e suporte upstream para gerenciadores de arquivos. A política também exige divulgação do uso de IA e proíbe conteúdo assistido por IA no manifesto e nas interações automatizadas de submissão. A autoria humana precisa ser independente; reescrever ou copiar o exemplo acima não remove essa condição.

1. Consolidar o protótipo: incluir conversores, definir Lixeira e integrações, executar a matriz de testes da tabela e resolver limitações de uso cotidiano.
2. Definir e manter a versão estável suportada, com releases rastreáveis, responsável por atualizações e revisão dos componentes/licenças incluídos.
3. Revisar manualmente AppStream, descrição, OARS e inglês/português. Publicar screenshots reais com dados sintéticos e URLs permanentes; os screenshots e o histórico de release da loja ainda estão pendentes.
4. Você escreve o manifesto de submissão a partir da documentação oficial, com fontes fixadas e builds offline no SDK; resolve as dependências Cargo com a ferramenta oficial e revisa o resultado. Não use o manifesto didático como base.
5. Testar o build, instalar e rodar o linter com `org.flatpak.Builder`, seguindo a [submissão oficial](https://docs.flathub.org/docs/for-app-authors/submission). Justificar permissões e resolver ou obter análise das exceções necessárias.
6. Você faz o fork e a submissão humana à branch `new-pr`, preenche a divulgação de IA com base no histórico real e responde aos revisores com suas próprias palavras. Não há PR, texto de submissão ou aprovação preparados automaticamente aqui.
7. Após aceitação, verificar a propriedade da conta/repositório com `janderpt`, publicar pelo processo da loja e manter o runtime e os módulos atualizados. Aceitação não é garantida.

Políticas consultadas em 2026-11-12. Esta é documentação técnica do projeto, não um texto de PR ou uma declaração de revisão humana concluída.

## English summary

This is a local **experimental Flatpak**, not a Flathub release. Install the bundle with `flatpak install --user FILE.flatpak`, run `flatpak run --branch=experimental io.github.janderpt.linux-system-explorer`, and uninstall that branch with `flatpak uninstall --user io.github.janderpt.linux-system-explorer//experimental`. The Freedesktop 26.08 runtime is downloaded separately. Use a maintained Flatpak installation with xdg-dbus-proxy 0.1.9 or newer; the original Ubuntu 24.04 proxy 0.1.5 cannot handle the zbus 6 protocol, regardless of runtime version. Preferences are sandbox-specific and neither installation nor removal changes the default file manager. The unsigned local bundle has no automatic repository updates.

Images/text, GStreamer media and libarchive use embedded/runtime components. PDF, Office and formatted HTML/Markdown converters remain to be packaged. Opening files uses the desktop portal; terminal commands run inside the sandbox. New remote connections, local Samba management and FileManager1 host registration are not enabled. Trash parity, mounted paths, DND and both desktop/window-system combinations require real-session validation. The build report records actual runtime capabilities and permissions. Keep the native package for full existing integrations.

The local JSON example is AI-assisted educational material, repackaging an existing staging directory, and is explicitly unsuitable for a Flathub submission or its starting point. The eventual store manifest and submission/review interactions must be independently human-authored. A stable, maintained release, a source/offline SDK build, complete converters, portal/desktop tests, metadata/screenshots, license review and manual store review are still required. See the linked official documentation rather than copying this local example into a submission.
