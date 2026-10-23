# Estado UI–daemon: snapshots e notificações

## Fluxo atual

`KWin → daemon → StatusChanged → cliente nativo → Channel Tauri → React`.

Depois que `ProfileSwitcher.handle` conclui com sucesso, o daemon publica o estado
no `StatusStore` e emite um sinal com o snapshot completo. Receber uma mensagem de
foco sem processá-la não publica um perfil como aplicado. No modo --kde, o remapper é a sessão de mouse, inicialmente pausada. O campo
simulation é true quando não há captura ativa e false quando o mouse está sendo
remapeado. Modos demo/stdin continuam usando DryRunRemapper. A seleção de um perfil
não ativa captura por si só. Veja [native-input.md](native-input.md).

A interface assina o serviço uma vez. Recebe um snapshot inicial, seguido por
notificações; não chama `get_daemon_status` a cada segundo. Não foi medida a latência
total na sessão do usuário, mas não há mais espera periódica de um segundo no painel.
O monitor KWin continua verificando a saúde/dono do compositor a cada 500 ms enquanto
aguarda foco; eventos de foco acordam o monitor imediatamente, sem esperar esse prazo.

## Contrato D-Bus

Serviço `org.appbuttons.Remap`, caminho `/org/appbuttons/Remap`, interface
`org.appbuttons.Remap.Status1`. Métodos e sinais são somente leitura de estado:

| Nome | Tipo | Conteúdo |
| --- | --- | --- |
| GetStatus | Método | Snapshot legado `(u, b, s, s, t)` |
| GetStatusWithHistory | Método | Snapshot completo `(u, b, s, s, t, a(ts))` |
| GetStatusWatchVersion | Método | Versão das notificações: `1` |
| StatusChanged | Sinal | Mesmo snapshot completo de GetStatusWithHistory |

Os campos, na ordem, são versão do protocolo (`1`), modo de simulação (`true`),
ID de perfil (vazio quando nenhum), estado de foco, revisão u64 e histórico.
Estados de foco: `waiting`, `focused`, `no_active_window`, `stopped`.
O histórico contém até 12 pares `(revision, profile_id)`, do mais antigo ao mais
recente, sem duplicar perfis consecutivos. É mantido somente em memória.

A revisão aumenta após cada evento consumido, mesmo sem mudança de perfil. O
snapshot e o histórico correspondem à mesma revisão. `default / focused` indica
uma janela sem regra correspondente; `default / waiting` aguarda o primeiro foco.
No reset explícito/erro tratado, o daemon limpa o perfil, mantém o histórico e emite
`stopped`. Saída por sinal ainda não tem reset explícito; o serviço desaparece e o
cliente mostra indisponibilidade. Um novo processo começa com revisão/histórico novos.

GetStatus e GetStatusWithHistory permanecem disponíveis para diagnóstico. O cliente
de consulta faz fallback para GetStatus apenas em UnknownMethod, indicando histórico
indisponível. A assinatura de eventos exige GetStatusWatchVersion=1: um daemon antigo
mostra aviso de reinício, sem reintroduzir polling no painel.

## Assinatura, segurança e reconexão

O cliente registra os sinais de estado e `NameOwnerChanged` antes de consultar o
dono do serviço e ler o snapshot inicial. Mensagens antigas já cobertas pela revisão
do snapshot são ignoradas. A fila de estado tem capacidade configurada de 32 sinais;
não há promessa de entrega ilimitada sob sobrecarga.

Somente sinais cujo remetente único corresponde à instância atual do daemon são
aceitos. Sinais de outro processo ou do dono anterior são ignorados. A saída/troca
de dono remove o estado antigo da UI; uma nova instância é inicializada novamente,
aceitando uma revisão menor. O daemon ausente não causa consultas periódicas: o
cliente aguarda o aviso de chegada do serviço. Se o próprio bus falhar, há tentativa
de reconexão a cada segundo, cancelável, sem expor snapshots anteriores como atuais.

As chamadas iniciais têm timeout de dois segundos. Modo nativo, versão incompatível,
metadados inválidos e históricos fora de ordem/limite são rejeitados. O frontend não
fornece endereços ou nomes arbitrários ao comando. O D-Bus de estado não escreve perfis. Comandos Tauri separados carregam e salvam
a configuração local; não iniciam processos nem injetam eventos de entrada.

A fronteira de confiança é a sessão do usuário: não existe atestação criptográfica
da identidade do daemon. O estado expõe IDs de perfis, sem títulos, app IDs,
executáveis ou teclas. Controle de taxa, bloqueio de sessão e entrada nativa permanecem
pendentes antes de qualquer remapeamento físico.

## Tauri e React

`start_daemon_status` recebe um Channel e retorna um ID de assinatura. O canal envia
`{kind: "status", status: ...}` ou `{kind: "unavailable", message: ...}`.
`stop_daemon_status` cancela somente aquela assinatura. Existem no máximo quatro
assinaturas temporárias da janela principal, permitindo sobreposição curta de
StrictMode/HMR. IDs distintos evitam que uma limpeza atrasada cancele uma nova.
Fechamento ou recarregamento do documento cancela todas as assinaturas da janela.

O DTO serializa revisão e revisões do histórico como strings decimais, preservando
u64 no JavaScript. A UI recebe eventos diretamente, retira snapshots ao desconectar
e ignora respostas após desmontagem. Se o registro terminar após a desmontagem, o
handle recém-chegado é cancelado. Não existe timer de consulta no React.

`get_daemon_status` continua como comando de diagnóstico, fora do fluxo de atualização.
No navegador não há conexão com o daemon; é necessário o shell Tauri, atualmente
com transporte de estado implementado somente em Linux.

O inventário de dispositivos é uma consulta independente, `Devices1.GetDevices`,
registrada no mesmo caminho. O comando Tauri `list_devices` é usado na conexão
inicial e por atualização manual da lista. Ele não introduz polling no fluxo de
perfis. Consulte [devices.md](devices.md).

## Validação

O teste com daemon real em bus privado cobre snapshot inicial, mudanças por sinal,
histórico, sinal falsificado ignorado, saída do serviço, novo dono/revisão, aviso para
serviço legado sem notificações e cancelamento do watcher. Os testes do daemon
cobrem também falha de publicação com reset do estado simulado.

Os testes JS cobrem eventos rápidos sem polling, desconexão, nova revisão após
reinício, histórico, falha de registro e limpeza após desmontagem/registro atrasado.
Com o Vite aberto, `http://127.0.0.1:1420/tests/events-browser.html` executa uma fixture
com eventos sintéticos (sem daemon) e deve mostrar `PASS` imediatamente.

O build web e `cargo check` do shell Tauri passaram. O novo fluxo deve ser conferido
no desktop após reiniciar o daemon e a interface; o agente não interrompeu os processos
do usuário nem alterou o script KWin nesta etapa.

## Perfis persistidos

O daemon `--kde` verifica o arquivo versionado durante a espera de foco. Uma nova
configuração válida reaplica o mapa simulado, mesmo com o mesmo ID de perfil, e
publica o snapshot por StatusChanged. Recarregar regras sem mudar o perfil não
cria uma entrada duplicada no histórico. O IPC de estado continua somente leitura.


A captura explícita usa uma interface separada, `org.appbuttons.Remap.Capture1`,
com resultado temporário de um único app ID. Não altera o formato dos snapshots de
estado, nem acrescenta títulos/app IDs ao histórico. Consulte [profiles.md](profiles.md).


## Nomes visíveis na interface

O protocolo continua transmitindo IDs estáveis. O frontend resolve esses IDs para
os nomes da configuração carregada, tanto no perfil selecionado quanto no histórico.
Carregar, recarregar ou salvar perfis atualiza os rótulos. Renomear um perfil também
atualiza o nome mostrado nas entradas anteriores do histórico; não há snapshot de
nomes antigos no protocolo. Um ID ausente na configuração aparece como “Perfil não
encontrado”; falha ao carregar os nomes aparece como “Nome indisponível”. Nenhum
UUID é usado como rótulo principal, e nenhuma regra de seleção foi alterada.
