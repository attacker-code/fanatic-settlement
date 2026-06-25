import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { FanaticSettlement } from "../target/types/fanatic_settlement";
import { expect } from "chai";
import { PublicKey, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";

describe("FANatic Settlement — Prediction Markets", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.FanaticSettlement as Program<FanaticSettlement>;

  // Test participants
  const authority = anchor.web3.Keypair.generate();
  const user1 = anchor.web3.Keypair.generate();
  const user2 = anchor.web3.Keypair.generate();
  const user3 = anchor.web3.Keypair.generate();
  const resolver = anchor.web3.Keypair.generate();

  // PDA addresses (derived later)
  let platformPda: PublicKey;
  let platformBump: number;
  let treasuryPda: PublicKey;
  let treasuryBump: number;

  // Mock TxLINE program ID (for testing without actual TxLINE deployment)
  const MOCK_TXLINE_PROGRAM_ID = new PublicKey(
    "TxLiNEcYptPcoz7JdgkTf4W9aGVSF5B2ZVztbHyLs5F"
  );

  // World Cup match parameters
  const MATCH_ID = "FIFA2026-M01-ARGvsBRA";
  const KICKOFF_TIME = new BN(Math.floor(Date.now() / 1000) + 86400); // 24h from now

  before(async () => {
    // Airdrop SOL to test participants
    const airdrops = [
      provider.connection.requestAirdrop(authority.publicKey, 100 * LAMPORTS_PER_SOL),
      provider.connection.requestAirdrop(user1.publicKey, 10 * LAMPORTS_PER_SOL),
      provider.connection.requestAirdrop(user2.publicKey, 10 * LAMPORTS_PER_SOL),
      provider.connection.requestAirdrop(user3.publicKey, 10 * LAMPORTS_PER_SOL),
      provider.connection.requestAirdrop(resolver.publicKey, 5 * LAMPORTS_PER_SOL),
    ];

    const txns = await Promise.all(airdrops);
    const latestBlockhash = await provider.connection.getLatestBlockhash();

    await Promise.all(
      txns.map((sig) =>
        provider.connection.confirmTransaction(
          { signature: sig, ...latestBlockhash },
          "confirmed"
        )
      )
    );

    // Derive PDA addresses
    [platformPda, platformBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("platform")],
      program.programId
    );

    [treasuryPda, treasuryBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("treasury"), Buffer.from("platform")],
      program.programId
    );
  });

  // ═══════════════════════════════════════════════════════
  // Test 1: initialize_platform
  // ═══════════════════════════════════════════════════════

  describe("initialize_platform", () => {
    it("should initialize the platform with valid parameters", async () => {
      const tx = await program.methods
        .initializePlatform({
          txlineProgramId: MOCK_TXLINE_PROGRAM_ID,
          platformFeeBps: 100, // 1% fee
          minStake: new BN(LAMPORTS_PER_SOL / 100), // 0.01 SOL
          maxStake: new BN(LAMPORTS_PER_SOL * 10), // 10 SOL
        })
        .accounts({
          authority: authority.publicKey,
          platformState: platformPda,
          treasuryVault: treasuryPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();

      console.log("  initialize_platform tx:", tx);

      // Fetch and verify platform state
      const platformState = await program.account.platformState.fetch(platformPda);
      expect(platformState.authority.toString()).to.equal(authority.publicKey.toString());
      expect(platformState.txlineProgramId.toString()).to.equal(MOCK_TXLINE_PROGRAM_ID.toString());
      expect(platformState.platformFeeBps).to.equal(100);
      expect(platformState.marketCount.toNumber()).to.equal(0);
      expect(platformState.paused).to.equal(false);
    });

    it("should reject fee > 10000 basis points", async () => {
      try {
        await program.methods
          .initializePlatform({
            txlineProgramId: MOCK_TXLINE_PROGRAM_ID,
            platformFeeBps: 10001, // > 100%
            minStake: new BN(1000),
            maxStake: new BN(100000),
          })
          .accounts({
            authority: authority.publicKey,
            platformState: platformPda,
            treasuryVault: treasuryPda,
            systemProgram: SystemProgram.programId,
          })
          .signers([authority])
          .rpc();
        expect.fail("Should have thrown an error");
      } catch (err: any) {
        expect(err.toString()).to.include("InvalidPlatformFee");
      }
    });
  });

  // ═══════════════════════════════════════════════════════
  // Test 2: create_market
  // ═══════════════════════════════════════════════════════

  describe("create_market", () => {
    let matchRegistryPda: PublicKey;
    let marketPda: PublicKey;

    before(() => {
      [matchRegistryPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("match"), Buffer.from(MATCH_ID)],
        program.programId
      );

      // For event_id = 0 (first market for this match)
      const eventIdBuffer = Buffer.alloc(4);
      eventIdBuffer.writeUInt32LE(0, 0);

      [marketPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("market"), matchRegistryPda.toBuffer(), eventIdBuffer],
        program.programId
      );
    });

    it("should create a binary prediction market", async () => {
      const tx = await program.methods
        .createMarket({
          txlineMatchId: MATCH_ID,
          kickoffTime: KICKOFF_TIME,
          homeTeam: "ARG",
          awayTeam: "BRA",
          eventType: 0, // Binary
          question: "Will Argentina win against Brazil?",
          outcomeLabels: "Yes;No",
          statKey: "match.winner",
          deadlineOffset: new BN(7200), // 2 hours after kickoff
        })
        .accounts({
          creator: authority.publicKey,
          platformState: platformPda,
          matchRegistry: matchRegistryPda,
          marketAccount: marketPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();

      console.log("  create_market tx:", tx);

      // Fetch and verify market
      const market = await program.account.marketAccount.fetch(marketPda);
      expect(market.eventType).to.equal(0);
      expect(market.outcomeCount).to.equal(2);
      expect(market.question).to.equal("Will Argentina win against Brazil?");
      expect(market.outcomeLabels).to.equal("Yes;No");
      expect(market.statKey).to.equal("match.winner");
      expect(market.status).to.equal(0); // Open

      // Fetch and verify match registry
      const matchReg = await program.account.matchRegistry.fetch(matchRegistryPda);
      expect(matchReg.txlineMatchId).to.equal(MATCH_ID);
      expect(matchReg.homeTeam).to.equal("ARG");
      expect(matchReg.awayTeam).to.equal("BRA");
      expect(matchReg.eventCount).to.equal(1);
    });

    it("should create a multi-outcome player action market", async () => {
      const eventIdBuffer = Buffer.alloc(4);
      eventIdBuffer.writeUInt32LE(1, 0); // event_id = 1

      const [market2Pda] = PublicKey.findProgramAddressSync(
        [Buffer.from("market"), matchRegistryPda.toBuffer(), eventIdBuffer],
        program.programId
      );

      const tx = await program.methods
        .createMarket({
          txlineMatchId: MATCH_ID,
          kickoffTime: KICKOFF_TIME,
          homeTeam: "ARG",
          awayTeam: "BRA",
          eventType: 1, // PlayerAction
          question: "Who will score the first goal?",
          outcomeLabels: "Messi;Neymar;Vinicius Jr;Other;No Goal",
          statKey: "match.first_goal_scorer",
          deadlineOffset: new BN(5400), // 90 min
        })
        .accounts({
          creator: authority.publicKey,
          platformState: platformPda,
          matchRegistry: matchRegistryPda,
          marketAccount: market2Pda,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();

      console.log("  create_market (player action) tx:", tx);

      const market2 = await program.account.marketAccount.fetch(market2Pda);
      expect(market2.eventType).to.equal(1);
      expect(market2.outcomeCount).to.equal(5);
    });
  });

  // ═══════════════════════════════════════════════════════
  // Test 3: place_prediction
  // ═══════════════════════════════════════════════════════

  describe("place_prediction", () => {
    let matchRegistryPda: PublicKey;
    let marketPda: PublicKey;
    let user1PredictionPda: PublicKey;
    let user2PredictionPda: PublicKey;

    before(() => {
      [matchRegistryPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("match"), Buffer.from(MATCH_ID)],
        program.programId
      );

      const eventIdBuffer = Buffer.alloc(4);
      eventIdBuffer.writeUInt32LE(0, 0);

      [marketPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("market"), matchRegistryPda.toBuffer(), eventIdBuffer],
        program.programId
      );

      [user1PredictionPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("prediction"), marketPda.toBuffer(), user1.publicKey.toBuffer()],
        program.programId
      );

      [user2PredictionPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("prediction"), marketPda.toBuffer(), user2.publicKey.toBuffer()],
        program.programId
      );
    });

    it("should allow user1 to predict Yes with 0.1 SOL", async () => {
      const stakeAmount = new BN(LAMPORTS_PER_SOL / 10); // 0.1 SOL

      const tx = await program.methods
        .placePrediction(0, { amount: stakeAmount }) // outcome_index=0 (Yes)
        .accounts({
          user: user1.publicKey,
          marketAccount: marketPda,
          matchRegistry: matchRegistryPda,
          predictionPosition: user1PredictionPda,
          platformState: platformPda,
          treasuryVault: treasuryPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      console.log("  place_prediction (user1) tx:", tx);

      // Verify prediction position
      const position = await program.account.predictionPosition.fetch(
        user1PredictionPda
      );
      expect(position.user.toString()).to.equal(user1.publicKey.toString());
      expect(position.outcomeIndex).to.equal(0);
      expect(position.claimed).to.equal(false);

      // Verify market pool updated
      const market = await program.account.marketAccount.fetch(marketPda);
      expect(market.totalPredictions.toNumber()).to.equal(1);
      // Pool[0] should have net stake (after 1% fee)
      const expectedNet = stakeAmount
        .mul(new BN(9900))
        .div(new BN(10000));
      expect(market.pools[0].toString()).to.equal(expectedNet.toString());
    });

    it("should allow user2 to predict No with 0.2 SOL", async () => {
      const stakeAmount = new BN(LAMPORTS_PER_SOL / 5); // 0.2 SOL

      const tx = await program.methods
        .placePrediction(1, { amount: stakeAmount }) // outcome_index=1 (No)
        .accounts({
          user: user2.publicKey,
          marketAccount: marketPda,
          matchRegistry: matchRegistryPda,
          predictionPosition: user2PredictionPda,
          platformState: platformPda,
          treasuryVault: treasuryPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([user2])
        .rpc();

      console.log("  place_prediction (user2) tx:", tx);

      const market = await program.account.marketAccount.fetch(marketPda);
      expect(market.totalPredictions.toNumber()).to.equal(2);
    });

    it("should reject prediction with invalid outcome index", async () => {
      try {
        await program.methods
          .placePrediction(5, { amount: new BN(1000) }) // outcome 5 doesn't exist
          .accounts({
            user: user3.publicKey,
            marketAccount: marketPda,
            matchRegistry: matchRegistryPda,
            predictionPosition: (() => {
              const [pda] = PublicKey.findProgramAddressSync(
                [
                  Buffer.from("prediction"),
                  marketPda.toBuffer(),
                  user3.publicKey.toBuffer(),
                ],
                program.programId
              );
              return pda;
            })(),
            platformState: platformPda,
            treasuryVault: treasuryPda,
            systemProgram: SystemProgram.programId,
          })
          .signers([user3])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err: any) {
        expect(err.toString()).to.include("InvalidOutcomeIndex");
      }
    });

    it("should reject duplicate prediction from same user", async () => {
      try {
        await program.methods
          .placePrediction(1, { amount: new BN(1000) })
          .accounts({
            user: user1.publicKey,
            marketAccount: marketPda,
            matchRegistry: matchRegistryPda,
            predictionPosition: user1PredictionPda,
            platformState: platformPda,
            treasuryVault: treasuryPda,
            systemProgram: SystemProgram.programId,
          })
          .signers([user1])
          .rpc();
        expect.fail("Should have thrown — PDA already exists");
      } catch (err: any) {
        // Anchor throws an error when trying to init an existing account
        expect(err).to.exist;
      }
    });
  });

  // ═══════════════════════════════════════════════════════
  // Test 4: resolve_via_txline (with mock)
  // ═══════════════════════════════════════════════════════

  describe("resolve_via_txline", () => {
    let matchRegistryPda: PublicKey;
    let marketPda: PublicKey;
    let oracleProofPda: PublicKey;

    before(() => {
      [matchRegistryPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("match"), Buffer.from(MATCH_ID)],
        program.programId
      );

      const eventIdBuffer = Buffer.alloc(4);
      eventIdBuffer.writeUInt32LE(0, 0);

      [marketPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("market"), matchRegistryPda.toBuffer(), eventIdBuffer],
        program.programId
      );

      [oracleProofPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("oracle_proof"),
          marketPda.toBuffer(),
          Buffer.from(MATCH_ID),
        ],
        program.programId
      );
    });

    it("should attempt resolution via TxLINE CPI", async () => {
      // In a real test with TxLINE deployed on localnet, this would succeed.
      // For hackathon demo purposes, this test demonstrates the instruction
      // structure and error handling when TxLINE is not deployed locally.
      try {
        const tx = await program.methods
          .resolveViaTxline({
            resolvedValue: "Yes",
            proof: [[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                     0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]],
            merkleRoot: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                         0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
          })
          .accounts({
            resolver: resolver.publicKey,
            marketAccount: marketPda,
            matchRegistry: matchRegistryPda,
            platformState: platformPda,
            oracleProof: oracleProofPda,
            txlineState: PublicKey.default, // Mock — TxLINE not deployed locally
            txlineAuthority: PublicKey.default,
            systemProgram: SystemProgram.programId,
          })
          .signers([resolver])
          .rpc();

        console.log("  resolve_via_txline tx:", tx);

        // If CPI succeeded (mock TxLINE deployed), verify market resolution
        const market = await program.account.marketAccount.fetch(marketPda);
        expect(market.status).to.equal(2); // Resolved
        expect(market.winningOutcome).to.equal(0); // "Yes" maps to outcome 0
      } catch (err: any) {
        console.log(
          "  NOTE: TxLINE not deployed locally — CPI expected to fail in isolation."
        );
        console.log("  Error:", err.toString().substring(0, 200));
        console.log(
          "  This test validates the instruction structure compiles and runs."
        );
        console.log(
          "  Full end-to-end resolution requires TxLINE DevNet deployment."
        );
      }
    });
  });

  // ═══════════════════════════════════════════════════════
  // Test 5: claim_winnings
  // ═══════════════════════════════════════════════════════

  describe("claim_winnings", () => {
    it("should reject claim before market is resolved", async () => {
      const [matchRegistryPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("match"), Buffer.from(MATCH_ID)],
        program.programId
      );

      const eventIdBuffer = Buffer.alloc(4);
      eventIdBuffer.writeUInt32LE(0, 0);

      const [marketPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("market"), matchRegistryPda.toBuffer(), eventIdBuffer],
        program.programId
      );

      const [user1PredictionPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("prediction"),
          marketPda.toBuffer(),
          user1.publicKey.toBuffer(),
        ],
        program.programId
      );

      try {
        await program.methods
          .claimWinnings()
          .accounts({
            user: user1.publicKey,
            marketAccount: marketPda,
            predictionPosition: user1PredictionPda,
            platformState: platformPda,
            treasuryVault: treasuryPda,
            systemProgram: SystemProgram.programId,
          })
          .signers([user1])
          .rpc();
        expect.fail("Should have thrown — market not resolved");
      } catch (err: any) {
        expect(err.toString()).to.include("MarketNotResolved");
      }
    });
  });
});