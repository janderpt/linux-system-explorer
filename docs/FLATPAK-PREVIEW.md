# Conversores de preview no Flatpak experimental

A [alpha.5 experimental com previews](releases/v0.1.0-alpha.5-flatpak.md) inclui os conversores no próprio pacote e o botão Sobre. Na release do GitHub, escolha o arquivo **flatpak-previews** e `FLATPAK-SHA256SUMS`. As variantes alpha.4 permanecem disponíveis como histórico, sem alterações nos pacotes nem na tag.

| Componente incluído | Versão | Uso |
| --- | --- | --- |
| Poppler | 26.11.0 | PDF, rasterização de páginas e miniaturas; `pdfunite` para preparar seleções para impressão |
| Dados Poppler | 0.4.12 | Mapas de caracteres, incluindo documentos CJK |
| LibreOffice | 26.8.3.1 | Writer, Calc, Impress, Draw e Math; formatos Microsoft atuais/antigos e OpenDocument |
| WeasyPrint | 70.0 | Apresentação estática de HTML e Markdown; preparação de imagens/texto para impressão |
| MIT Kerberos | 1.22.2 | Biblioteca exigida pelo LibreOffice upstream; sem ferramentas administrativas no pacote |

As dependências Python completas, incluindo Pillow, CFFI, fontes WOFF e hifenização, estão fixadas em [flatpak-converters.lock.json](../packaging/flatpak-converters.lock.json). Pango, Fontconfig, FreeType e demais bibliotecas de base vêm da Freedesktop Platform 26.08. Não são usados executáveis do host, `flatpak-spawn --host`, outra instalação Flatpak do LibreOffice ou acesso novo à rede/D-Bus. O runtime continua sendo instalado pelo Flatpak normalmente.

O motor LibreOffice vem do arquivo binário oficial relocável, sem alterações no código. São extraídos apenas os pacotes de conversão, fontes e recursos necessários, sem executar scripts Debian, instalar lançadores do Office ou fornecer Java/extensões. Poppler, Kerberos e extensões Python nativas são compilados no SDK; pacotes Python puros usam wheels universais. Este reempacotamento experimental **não é uma compilação completa de fontes para submissão à loja**.

Os limites, o cancelamento e as proteções existentes do explorador continuam ativos: perfil temporário separado para Office, macros/conteúdo ativo desativados, originais preservados e apresentação HTML sem JavaScript ou recursos remotos. Arquivos inválidos ou não suportados continuam usando os toasts e ícones de fallback. A paginação pode variar conforme as fontes disponíveis.

A alpha.5 **flatpak-previews** inclui a [correção D-Bus](DBUS-COMPATIBILITY.md), testada com os proxies 0.1.5 e 0.1.9. Os bundles anteriores permanecem disponíveis como histórico; os fontes dos conversores continuam no mesmo anexo `FLATPAK-PREVIEW-SOURCES.tar`, também listado nos novos checksums.

## Reproduzir

Use o ambiente de ferramentas de [FLATPAK.md](FLATPAK.md), com `dpkg-deb` para **extração**, Python 3.12+ e estes refs instalados na instalação Flatpak privada de build:

```sh
flatpak install --user flathub org.freedesktop.Platform//26.08 org.freedesktop.Sdk//26.08
python3 scripts/flatpak-converters.py fetch
python3 scripts/flatpak-converters.py build --jobs 8
cargo build --release --locked
python3 scripts/flatpak.py stage
python3 scripts/flatpak.py build
```

O download exige rede e confere SHA-256 para **todos** os arquivos; o build no SDK fica sem rede e não faz resolução de versões. Um arquivo corrompido no cache aborta o processo. O SDK usado é registrado por commit no relatório. Para reconstruir uma revisão antiga, conserve também esse SDK e o runtime registrados, em vez de assumir que a branch móvel preserva as mesmas ferramentas.

`--cache`, `--output`, `--work-dir` e `--jobs` permitem separar download, saída e trabalho do conversor. Por padrão são `build/flatpak-preview-downloads`, `build/flatpak-preview-converters` e `build/flatpak-preview-work`. O staging aceita `--converters CAMINHO` para importar outro payload verificado. Destinos devem estar vazios. O processo recusa checksums divergentes, arquiteturas/runtimes incompatíveis e colisões com arquivos do aplicativo.

O payload inclui `PREVIEW-CONVERTERS.json` com versões, SDK, lock e hashes dos arquivos. O staging verifica esse registro e os fontes correspondentes antes de importar o conteúdo. A exportação verifica os programas e o import do WeasyPrint sob a **Platform**, sem o SDK. O bundle final não inclui compiladores, pip de build ou os arquivos temporários de compilação.

## Licenças e fontes correspondentes

As licenças de Poppler, dados Poppler e Kerberos ficam em `/app/share/licenses/lse-preview/`; as de LibreOffice e seus componentes são preservadas em `/app/lib/libreoffice/{LICENSE,LICENSE.html,NOTICE}`. Os pacotes Python preservam seus metadados e diretórios de licenças em `/app/lib/python3.14/site-packages/`. O relatório e os dois locks também acompanham o pacote.

`FLATPAK-PREVIEW-SOURCES.tar` acompanha o payload e é copiado para a saída do bundle, com checksum. Contém o código oficial exato do LibreOffice, os arquivos externos referenciados pela sua revisão, Poppler, dados Poppler, Kerberos e todos os pacotes Python de execução, além deste procedimento e do script de build. A lista externa é um conjunto conservador que também inclui fontes não utilizados pelos motores selecionados. Os fontes de terceiros **não** são colocados no histórico Git do explorador.

Ao distribuir o novo bundle, distribua também esse arquivo de fontes no **mesmo local**, os locks, `FLATPAK-BUILD-INFO.json`, o relatório de validação e os checksums. Os fontes aumentam bastante o conjunto de downloads; são um anexo separado e não aumentam a instalação do aplicativo. Conserve também as licenças/fontes do aplicativo nativo que já fazem parte do staging. Atualizações dos conversores exigem rever versões, hashes, licenças e repetir os testes; o LibreOffice incluído não se atualiza pelo programa do host.

Referências upstream: [LibreOffice, binários e fontes](https://www.libreoffice.org/download-other/), [Poppler](https://poppler.freedesktop.org/), [MIT Kerberos](https://kerberos.org/dist/) e [WeasyPrint](https://pypi.org/project/weasyprint/). Os URLs específicos e checksums estão nos locks.

## Validação do sandbox

Execute `scripts/smoke-flatpak.py` com `--test-binary` e `--fixtures`, conforme [FLATPAK.md](FLATPAK.md). Com o payload presente, ele exige os conversores, executa os testes de PDF, DOC/DOCX/ODT, XLS/XLSX/ODS, PPT/PPTX/ODP, ODG/ODF, HTML/CSS, Markdown, miniaturas e preparação de impressão **dentro do app instalado**. Esses testes conferem conteúdo/páginas e preservação dos originais; não basta encontrar um comando em `PATH`. Nenhuma página é enviada a uma impressora.

O teste de instalação/atualização aceita um bundle anterior com a mesma versão textual, desde que o commit OSTree seja diferente, e confere a preservação das preferências. Xvfb valida o ciclo e os renderizadores no sandbox, sem certificar a sessão gráfica real KDE/GNOME ou a submissão ao Flathub.
